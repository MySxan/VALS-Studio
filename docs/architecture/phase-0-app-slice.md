# Phase 0：后台导入与预览会话

本切片遵守 SDD，未引入 ADR 或修改 Canonical Domain Model。
阶段初始规划包含桌面 UI；实施拆分后，本次交付可测试的应用服务和 JSON 示例，
Tauri commands、文件选择器、React/Zustand 和 Canvas 留给后续独立切片。

## Modules 与 public interfaces

新增 `vocal-app::{AppService, AppError}` 和独立 DTO 模块（由 crate 根导出）。
依赖现有 domain、audio、analysis-api/runtime、dsp；Serde 只用于应用 DTO。
没有新增 registry dependency，现有 THIRD_PARTY 清单继续适用。

```rust
AppService::default() -> AppService
start_import(&self, path: PathBuf) -> Result<String, AppError>
job_status(&self, id: &str) -> Result<JobStatus, AppError>
cancel_job(&self, id: &str) -> Result<(), AppError>
current_session(&self) -> Result<Option<SessionDto>, AppError>
waveform_slice(&self, session_id: &str, start: f64, end: f64, width: u32)
    -> Result<WaveformDto, AppError>
```

任务状态：Running → Succeeded / Failed，或 Running → Cancelling → Cancelled。
终态取消是幂等操作；不存在或已经被下一任务取代的 job ID 返回 UnknownJob。
只保留最近一次任务状态，不提供历史任务日志。

## Invariants

- 每个 service 最多一个后台导入；取消请求后仍 Busy，直到 worker 退出。
- 导入、快照验证、Decode、Waveform 都运行在后台线程。运行时锁独立于状态锁。
- 取消和最终发布在同一个状态锁内串行决定；取消先获得锁时禁止发布。
- 失败、取消不替换已有预览会话；成功用一个 Arc 原子替换整个会话。
- 预览会话不是可编辑工程，没有隐式保存、重命名或替换已打开工程的行为。
- 视口只读 waveform artifact；先克隆 Arc 再释放状态锁，不扫描 PCM。
- 拒绝非有限时间、倒序范围、0 或超过 4096 的宽度。
- 返回每声道最多 width+1 个聚合块。数据带 session ID 和 artifact hash。
- 已知过期 session ID 拒绝查询；在途查询仍可返回其不可变旧快照。
  后续 UI 必须同时检查 session ID 与请求序号，丢弃过期响应，并裁剪完整聚合块。
- 当前状态提供粗粒度轮询，没有百分比进度或事件推送。应用退出应调用 cancel_job；
  丢弃 service 的外部 clone 不隐式取消正在运行的 worker。

## Cache 与 analysis dependencies

应用组合现有默认 provider：WAV import → Decode → WaveformPeaks。
算法、confidence、provenance 和 artifact identity 均由统一 Analyzer 实现产生；
DTO 不引入模型字段或改变领域表示。更换 provider/version/model 仍由现有 cache key 失效规则控制。

每个 service 保留一个 128 MiB payload 预算、最多 64 项的 FIFO 内存缓存。
预算不包括 Arc 外部持有者、DTO 和临时解码缓冲。运行时重新验证源文件，即使命中缓存。
会话持有 waveform Arc；任务结束后不通过 DTO 暴露 PCM。平移缩放不创建分析缓存项。
取消可能保留此前已经完整完成的上游节点缓存，但不会发布取消后的会话或未完成节点。
无磁盘 cache、无新增分析种类、无 schema cache migration。

## Overrides / provenance / migration

本服务只创建独立预览会话，不接受工程修改操作，不写任何 user override。
SessionDto 保留完整 waveform provenance（provider、版本、model、settings、源和依赖 hash、
时间、runtime），以及 confidence kind、可选 score 和 explanation。
波形 DTO 以 artifact hash 引用这些证据。Measurement 的 score 为 null，不伪造概率。
工程 schema 保持 v2，v1→v2 migration 与拒绝未知字段策略不变。

## Tests 与运行

7 项新增测试覆盖真实后台导入和 JSON 输出、失败保留旧会话、
分析完成后取消优先于发布、预取消不缓存、重复导入缓存复用和过期会话、
无效视口参数、Busy/未知任务/过期完成防护。竞争测试直接控制发布边界，避免靠调度时机碰运气。

`cargo run -p vocal-app --example import_session --offline --locked`
运行真实 WAV → 后台任务 → 会话 → 两像素波形查询，并输出 JSON DTO；可加 `-- path/to.wav`。
这是命令行验收示例，尚无桌面窗口。

本机 GNU：fmt、Clippy（-D warnings）、59 项测试、五个示例通过。
Windows MSVC 缺 Windows SDK，尚无桌面编译或浏览器 Canvas 验证；远程 CI 未执行。
