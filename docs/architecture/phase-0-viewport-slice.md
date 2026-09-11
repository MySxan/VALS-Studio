# Phase 0 / 波形视口查询切片

状态：完成。依据 SDD §5.3、§15.1、§32、§43、§49.3。无核心边界变更。
前置为 [Decode → WaveformPeaks](phase-0-analysis-slice.md)，不新增 analyzer。

## Modules / public interfaces

新增 `vocal-dsp::viewport`，仅依赖既有 analysis artifact 与 typed time。

```rust
WaveformViewport::new(start: Seconds, end: Seconds, pixel_width: u32)
    -> Result<WaveformViewport, AnalysisError>;
query_waveform(artifact: &AnalysisArtifact, viewport: WaveformViewport)
    -> Result<WaveformSlice, AnalysisError>;
```

`WaveformSlice` 包含父 artifact_hash、原始 metadata、所选 frames_per_block、每声道 point 列表。
`WaveformPoint` 包含完整 block 的 Samples start/end 与 min/max/RMS。
不存在 PCM 全量数组、模型类型或 UI 状态。

## Invariants

- Seconds 已保证有限；viewport 要求 end>start，pixel_width 在 1..4096。
- 请求范围按音频区间裁剪，无交集返回每声道空列表，允许负预卷与音频末尾之外的请求。
- 选择第一个宽度不小于 `ceil(visible_frames / pixel_width)` 的 level。
- 音频和 block 采用半开区间，恰好结束于 block 边界不会多取下一个 block。
- 每声道最多 pixel_width+1 个完整 aggregate blocks；超上限/缺少足够粗 level 明确报错。
- 返回真实 aggregate span，尾块使用实际帧数；不伪造视口子区间的 min/max/RMS。
  渲染器应按自己的 viewport 裁剪完整 block，而不能声称这是部分 block 的精确统计。
- 时间到 sample 位置在浮点误差范围内吸附到整数边界（四个相对 epsilon），避免边界舍入多取 block。
- 只接受经过 artifact 构造校验的 Waveform payload；PCM 输入明确拒绝。
- 查询复杂度 O(level 数 + 返回点数)，不扫描 PCM；像素上限限制单次返回量。

## Cache / analysis dependencies

新增缓存：无；失效：无。父 waveform artifact hash 是唯一结果追溯引用。
平移/缩放重复查询现有 pyramid，不调用 Decode/WaveformPeaks，不产生新 artifact。
父 artifact 改变时调用方重新查询并更新显示；查询本身不持有全局状态。

## Overrides / provenance / schema

所有输入只读；这是已有 artifact 的显示投影，不是新推断结论。
confidence/provenance 保留在父 artifact，通过 artifact_hash 关联。
未读写用户 override，未新增持久化字段；项目 schema v2 和既有 migration 不变。

## Tests / 验收

新增 4 项集成测试覆盖像素→level 选择与返回上限、半开边界/实际尾块、
负预卷/空交集/越界、非法 viewport 和错误 artifact 类型。
示例 analyze_waveform 已包含 2 像素查询，输出每声道 2 blocks、512 frames/block。

2026-09-11 / Rust 1.98.1 / Windows GNU：全仓库 **52 项测试通过**，
fmt、Clippy `-D warnings` 和四个示例通过。复现：`./tools/check-core.ps1`。
尚无 Canvas/IPC/UI 集成、UI 性能实测或播放。远程 CI 未执行；MSVC 本地仍缺 Windows SDK。
