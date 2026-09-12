# Phase 0：Project-backed Desktop Workflow

## 边界与接口

`vocal-app` 持有一个 `OpenProject`，其 `domain: VocalProject` 是唯一语义事实。
路径、dirty、代次 generation、后台任务及每轨派生分析单独保存；没有新增 domain 字段或 disk schema。
`jobs` 模块复用 WAV importer、verify_source、AnalysisRuntime 和原有 Decode/Waveform provider。
Tauri commands 通过 `spawn_blocking` 调用应用服务，前端仅消费 Zod 校验的 DTO。

| 接口 | 参数 | 结果 |
| --- | --- | --- |
| current_project | 无 | WorkspaceDto（generation/project/job） |
| new_project | name, generation, discard | WorkspaceDto |
| open_project | path, generation, discard | WorkspaceDto |
| close_project | generation, discard | WorkspaceDto |
| save_project | projectId, generation, path? | WorkspaceDto；null path 为 Save，显式 path 为 Save As |
| start_import | projectId, generation, path | job ID |
| analyze_track | projectId, generation, trackId | job ID |
| job_status / cancel_job | id | JobStatus / void |
| waveform_slice | projectId, generation, trackId, start, end, width | WaveformDto |

旧 `current_session` 和 session-based IPC 已移除，未将 preview session 身份引入领域。
generation 每次 New/Open/Close 递增，解决同一 project UUID 被关闭重开时的过期响应问题。

## Invariants 与失败语义

- Import 在临时工作结果中完成注册和分析，然后在取消/发布共用的状态锁内调用 `attach_audio`。
  只有完全成功才挂接并标记 dirty；失败或取消不产生半挂接实体。多个导入保留已有轨道。
- Open 只走 ProjectStore 元数据 load；不访问音频，不自动保存 migration。打开失败保留原工程。
  UI 选中轨道后才请求 analyze_track，先 verify_source，再运行原有 pipeline。
- 源状态为 unchecked/analyzing/ready/offline/changed/error；该状态仅在应用层，不写进工程事实。
  离线/变化不阻止加载和保存元数据，不自动修改 source hash。每次重新核验先撤销旧派生结果。
- New 工程为 dirty；成功导入为 dirty；分析完成不改变 dirty。Save 原子提交成功才清 dirty、更新路径。
- 保存与工程变更串行，活动分析期间拒绝 Save/New/Open/Close，取消后等待 worker 终态。
  Store 仍提供原子替换而不是跨进程冲突检测；Save As 保持已有实体 ID。
- New/Open/Close 对 dirty 工程要求显式 discard；取消文件选择不丢弃工程。
  原生窗口关闭也调用 close guard；后台任务运行时先在 UI 取消并等待完成。
- 保存路径要求 `.vocalproj`，且拒绝覆盖已识别的关联音频源路径。
- UI 保留所有轨道 DTO，仅选择一个进行显示；不从 UI 摘要重建工程。
  请求序号、project/track/generation 和 artifact hash 一起防止过期波形回填。

## Cache、provenance 与 schema

无新分析种类、无算法版本变化，仍是 Decode → WaveformPeaks。
打开新工程清理其应用派生状态；运行时缓存跨工程保留，以 source/model/provider/settings/dependency identity 命中。
保存不触发分析，也不序列化 waveform/confidence/provenance；这些证据随派生 artifact 存在。
重开后由 provider 重新取得派生结果，不能作为用户编辑事实保存。原始观察、resolved value 和未来 user override 边界未合并。
Canonical Domain Model、schema v2、v1→v2 migration、拒绝未知字段策略和原子写入实现均未修改。

## 验证范围

核心应用测试覆盖完整保存重开、身份保持、多轨保存、失败/取消不挂接、clean/dirty 状态、
源离线/变化仍打开、错误保存/损坏打开保留旧状态、代次保护、Busy、viewport 边界、
过期 worker 以及应用层 v1 load 不写回/显式 Save 升级 v2。
Tauri MockRuntime 经真实命令 handler、ProjectStore、WAV 和分析执行完整闭环，检查 camelCase 和过期/非法参数。
前端端口集成测试覆盖 New/Import/Save/Close/Open、重算、Save As/放弃确认取消、source 状态、失败/取消、
过期 generation、视口响应、重载恢复任务、通信失败重试、保存失败和真实 Rust fixture DTO。
这些测试不替代实际 Windows 原生文件对话框和关闭事件验收。

## 运行

`./tools/check-core.ps1` 检查核心；`apps/desktop` 中运行 `npm test` 检查前端。
`./tools/build-desktop.ps1` 构建前端、检查原生 Clippy、运行 IPC 测试、生成原生开发程序。
可执行文件：`apps/desktop/src-tauri/target/debug/vals-desktop.exe`。
示例 `cargo run -p vocal-app --example import_session` 保留历史示例名称，现输出 project-backed workspace/waveform DTO。
