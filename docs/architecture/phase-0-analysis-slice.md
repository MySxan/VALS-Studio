# Phase 0 / Decode → WaveformPeaks 分析切片

状态：本切片完成；未达到 M0。依据 SDD §3、§9–15、§49、§53–54。无核心边界变更。

## Modules / public interfaces

- domain::analysis：Confidence、ConfidenceKind、ModelIdentity、Provenance。
- domain::signal：不可变 interleaved Pcm；每声道 Peak、WaveformLevel、WaveformPyramid。
- vocal-analysis-api：Analyzer、AnalysisContext、AnalysisArtifact、descriptor、取消、进度和 cache key。
- vocal-analysis-runtime：同步单节点执行器、显式依赖校验、有界 FIFO 内存缓存。
- vocal-audio::DecodeAnalyzer：Symphonia WAV/PCM provider，固定快照解码。
- vocal-dsp::WaveformAnalyzer：独立的 min/max/RMS pyramid provider。

依赖：audio/dsp/runtime → analysis-api → domain；domain 不依赖 provider/UI/存储。
原 `vocal_audio::CancellationToken` 重导出统一 token，原导入调用代码保持兼容。

```rust
trait Analyzer: Send + Sync {
    fn descriptor(&self) -> AnalyzerDescriptor;
    fn dependencies(&self) -> Vec<AnalysisKind>;
    fn supports(&self, ctx: &AnalysisContext) -> SupportLevel;
    fn normalized_settings(&self, ctx: &AnalysisContext) -> Result<BTreeMap<String,String>, AnalysisError>;
    fn validate_inputs(&self, ctx: &AnalysisContext, cancel: &CancellationToken) -> Result<(), AnalysisError>;
    fn run(&self, ctx: AnalysisContext, cancel: CancellationToken, progress: ProgressSink)
        -> Result<AnalysisArtifact, AnalysisError>;
}
AnalysisRuntime::new(payload_budget: usize);
AnalysisRuntime::execute(&mut self, analyzer: &dyn Analyzer, ctx, cancel, progress)
    -> Result<Execution, AnalysisError>;
```

Execution 包含 `Arc<AnalysisArtifact>` 与 cache_hit。Artifact 私有字段，通过校验构造器创建；
提供 kind/cache_key/artifact_hash/provenance/confidence/payload 只读读取。

## Invariants

- 每个节点声明 kind 和依赖；执行器拒绝缺失、多余、顺序错误或不同 source hash 的依赖。
- Decode 不混音、截幅或重采样，输出原声道 interleaved f32，拒绝非有限值。
- 解码从经 SHA-256 核验的临时快照读取；核对实际 rate、channels、frames；不把 EOF 当作完整性证明。
- PCM payload 上限 64 MiB，超过明确报 LimitExceeded。不是流式长音频解码的最终实现。
- 波形每声道独立，默认 256/512/1024…帧每块，逐层倍增直到一个 block。
  支持规范化 base_frames（256..65536 的二次幂）；未知/非规范 settings 拒绝。
- min/max 为实际 extrema；RMS 使用 f64，合并按真实帧数加权，尾块不补零。
- 确定性 measurement 的 probability score=None，解释明确；不以 1.0 假装概率校准。
- provenance 包括 analyzer/provider/version、可选 model id/version/hash、settings/hash、source hash、
  dependency hashes、timestamp 和 OS/architecture；artifact hash 不包含 timestamp。
- 在 I/O、解码 packet、pyramid block、artifact hashing 与发布边界检查取消。
  取消/失败不发布新 artifact；已提交的上游 artifact 不因此删除。OS 阻塞调用不可立即中断。

## Cache / dependencies

显式链：source hash → Decode → WaveformPeaks。调用方按拓扑顺序提交两个独立节点，
不是一个 monolithic analyzeAudio；当前尚无并行 DAG 自动调度器/任务池。

key 使用 SHA-256、版本前缀和长度分隔编码，包含 analyzer/provider/version/kind、model 身份/hash、
source content hash/size/metadata、规范化 settings hash、依赖 artifact hashes。
不使用项目 ID、实体 ID、路径或 mtime 作为内容身份。省略 base_frames 与显式 256 规范化为同值。
provider/model/version/settings/上游 artifact 变化会产生不同 key；旧结果仍作为不可变历史内容存在。

内存缓存按插入顺序淘汰，最多 64 个条目，保留 payload 总量不超过配置预算；超预算单结果返回但不缓存。
此预算不包含对象/索引开销、临时解码缓冲与外部 Arc 持有的数据，不是进程总内存承诺。
Decode 命中前也校验原文件；waveform 可只使用已固定的 Decode artifact，无需再次读取源路径。
没有新增磁盘 artifact/cache 格式；重启后重新计算。进度回调不得阻塞执行线程。

## Overrides / schema

Analyzer 不接收可变 Project/Revision；执行与缓存不会覆盖用户编辑或删除原始结果。
完整 Revision 和 raw observation 持久化仍未实现，不能据此宣称已经支持其全部编辑工作流。
artifact 暂只在内存，未新增项目持久化字段；schema 保持 v2，v1→v2 migration 不变。
未来持久化 artifact 时须定义独立版本/校验/原子写入，不能直接序列化内部 struct 布局。

## Tests / 验收

新增 12 项集成测试覆盖：逐采样 fixture 解码、声道保留、provenance/confidence、
两节点依赖与默认 settings、加权尾块 RMS、错误依赖/settings、缓存命中前源内容变化、
多阶段取消与发布边界、容量上限和淘汰、实际帧数不符、可替换测试 provider、版本/model hash 失效、
非法数值与伪造 provider 结果拒绝、可重现 artifact hash。

2026-09-11 / Rust 1.98.1 / Windows GNU：全仓库 **48 项测试**、fmt、Clippy `-D warnings`、
四个示例通过。执行 `./tools/check-core.ps1`。
示例 `cargo run -p vocal-analysis-runtime --example analyze_waveform` 可附带 `-- path/to/vocal.wav`。
MSVC 本地缺 Windows SDK，远程三平台 CI 未执行；无 UI 渲染/播放验收。

Symphonia 调用遵循 [0.5.5 官方解码流程](https://docs.rs/symphonia/0.5.5/symphonia/)。
未新增第三方包版本；复用已有 Symphonia、sha2、tempfile。THIRD_PARTY.toml 的版本清单不变。
