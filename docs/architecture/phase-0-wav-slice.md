# Phase 0 / 关联 WAV 导入与 schema v2

本页保留该切片的历史契约。后续已新增 [独立 Decode/波形分析](phase-0-analysis-slice.md)，
原关联导入仍仅注册容器元数据，职责不变。

状态：本切片已完成，完整 Phase 0/M0 未完成。依据 SDD §7.6–7.7、§14、§28、§53–56。
无核心架构变更，未引入偏离 SDD 的 ADR。

## 范围与模块

闭环：WAV 文件 → 内容 hash 与容器元数据 → AudioSource/VocalTrack → 保存 → 重开 → 核验源文件。
本次导入是文件注册，不创建推断结果，也不把容器探测成功视为 PCM 已解码/可播放。

- `vocal-domain::audio`：通用内容 SHA-256、AudioUri、AudioMetadata、AudioSource、ChannelMode、VocalTrack。
  不暴露 Symphonia 类型，不依赖音频库或 Serde。
- `vocal-domain::project`：sources/tracks 集合、身份和引用验证、原子挂接。
- `vocal-audio::io`：64 KiB 分块复制/hash、共享取消标记、源文件核验。
- `vocal-audio::import`：WAV 快照和 Symphonia 探测适配器。
- `vocal-project`：私有 v2 DTO、真实 v1→v2 migration、原子保存逻辑复用。

生产依赖方向：audio → domain；project → domain；time → domain。
project 的集成测试/示例依赖 audio；生产 persistence 不导入音频。

## Public interfaces

```rust
WavImporter::import_linked(path: impl AsRef<Path>, cancel: &CancellationToken)
    -> Result<AudioSource, AudioError>;
verify_source(source: &AudioSource, cancel: &CancellationToken) -> Result<(), AudioError>;
CancellationToken::{default, clone, cancel, is_cancelled};

AudioMetadata::new(rate: u32, channels: u16, frames: Samples) -> Result<Self, AudioDomainError>;
// metadata 提供 sample_rate/channels/frames/duration 读取。
AudioSource::new(id, uri, content_hash, size_bytes, metadata) -> Result<Self, AudioDomainError>;
// source 提供 id/uri/content_hash/size_bytes/metadata 读取。
VocalProject::attach_audio(source: AudioSource, track: VocalTrack) -> Result<(), AudioDomainError>;
VocalProject::sources(&self) -> &[AudioSource];
VocalProject::tracks(&self) -> &[VocalTrack];
VocalProject::with_audio(id, name, sources, tracks) -> Result<Self, AudioDomainError>;
```

既有 `ProjectStore::save/load` 接口不变。`MigrationRegistry::default` 现在注册 v1→v2；
新增 `empty()` 供受控测试/自定义链使用。`AudioError` 提供分类、message key 和错误链。

## Invariants

- 原文件只读；原始采样率、声道数保留。EqualPowerMono 仅是后续通道选择意图，未执行混音。
- 当前导入支持 RIFF/WAVE 的非空 mono/stereo 容器；不承诺 RF64、RIFX 或其他文件格式。
- 元数据是容器声明的 rate/channels/frame count。采样率、声道数、帧数必须为正；
  duration 从 Samples/rate 派生，不写冗余浮点持续时间。
- SHA-256 覆盖原始完整文件字节，不依赖文件名、修改时间或工程身份。
- 先流式复制并 hash 到临时快照，再从同一快照探测元数据；返回前重新读取原路径核验内容。
  临时磁盘占用约等于文件大小，正常返回/错误/取消均由文件 RAII 清理；不常驻整段音频内存。
- 取消检查位于分块 I/O 边界与探测前后，探测读取也检查取消；不能中断已进入 OS 的阻塞读取。
- import 不修改项目；attach 在任何字段变化前验证新增 source/track。ID 不能碰撞，track 必须引用 source。
  project 恢复时同样验证全体 ID 唯一性与引用完整性。
- load 不访问音频路径。缺失源仍保留全部元数据与 ID；verify 返回 I/O 错误或 SourceChanged。
- 路径在 domain 中为字符串，允许打开来自其他 OS 的工程；使用前 adapter 检查是否可在当前主机解析。
  当前核验仅使用绝对 fallback，relativePath 为受限的可选相对路径预留，尚不执行重定位。

## Cache / analysis dependencies

新增持久化分析 cache：无。临时快照是导入工作文件，不是可复用 artifact。
新增未来缓存输入：AudioSource.content_hash。source/track 改名不会改变该 hash。
未来 Decode/CanonicalAudio/波形/F0 缓存应依赖内容 hash、provider/version/settings 等；
通道模式和重采样策略属于后续 analyzer settings，须进入对应 cache key。
源内容变化不能沿用旧内容的分析；当前只报告 SourceChanged，不悄悄刷新 source 元数据或覆盖分析。

## Overrides / provenance / migration

未新增自动分析结果；容器元数据注册不是模型/启发式结论，因此没有虚构 confidence=1 或 analyzer provenance。
后续 PCM/DSP/AI 阶段须实现 SDD 的统一 Analyzer contract，包含 confidence/provenance、
显式依赖、provider replacement、可缓存与可取消；不得将本 importer 充当无契约 analyzer。

本阶段仍未实现 Revision 或原始 observation 持久化。未知 override/provenance 字段继续拒绝加载，
既有字段不被静默舍弃。新增 v2 的 sources/tracks 必填，v1→v2 先严格验证旧 DTO，
再添加空数组，保留 id/name；v1 重复字段在转换为 JSON Value 前拒绝。
迁移只在内存执行，save 才写 v2；原子替换和失败保留旧文件的测试继续有效。

## Tests / 结果

新增 12 项测试：7 项 audio、2 项 domain、3 项 persistence。
包含固定 mono/stereo fixtures 与外部独立计算的 SHA-256、已知 SHA-256 向量、
多块复制、读途中取消、文件改名/变化/缺失、非 WAV/截断/空输入、原文件不变、
原子挂接、ID 碰撞/悬空引用、无效元数据/hash/路径、v2 round-trip、离线源重开、
真实 v1→v2 migration、未知 v2 数据拒绝。

2026-09-11，Rust 1.98.1 / Windows GNU：**36 项测试全部通过**，fmt、Clippy `-D warnings`、
三个示例均通过。复现：`./tools/check-core.ps1`。
MSVC Windows SDK 缺失与远程三平台 CI 未执行的限制不变。

## 后续边界

未实现 PCM 解码/内容有效性逐帧检查、波形/播放、内嵌音频、relink UI、autosave、编辑历史。
verify 是检查时点的内容验证；它与之后重新打开文件之间仍有文件变化窗口。
未来分析必须读取固定快照或受控同一源，不能将一次 verify 当作永久有效的承诺。
输入文件被删除/替换后不会自动搜索磁盘或修改引用。

依赖：Symphonia 0.5.5 稳定 API 分支，仅启用 wav/pcm；SHA-256 使用 sha2 0.11。
版本与完整 SPDX 记录在 THIRD_PARTY.toml/Cargo.lock；没有 FFmpeg、云传输或模型权重。
参考 [Symphonia 0.5.5](https://docs.rs/symphonia/0.5.5/symphonia/)、
[SHA-2](https://docs.rs/sha2/0.11.0/sha2/)。
