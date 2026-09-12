# 当前状态

- 桌面工程闭环已实现：New / Open / Import WAV / Save / Save As / Close、轨道选择、dirty、后台 waveform、取消、缩放/平移、播放 transport/同步游标和 confidence/provenance Inspector。
- 同内容 Relink 保留 project/source/track 身份；失败或取消不改原关联和证据。支持工程相对路径：目录整体移动后可重开并核验源；相对候选 missing 才用 absolute fallback，changed 不静默旁路。
- Save / Save As 在完整 domain 副本上重算 URI，原子写入成功才提交应用状态；工程目录外的音频用绝对路径，不复制 WAV。UI 显示实际核验位置。未核验的跨目录 Save As 优先保留旧相对目标，详见路径切片契约。
- `VocalProject` 是唯一语义状态；generation、dirty、保存位置、分析与播放投影由 `vocal-app` 管理。稳定接口另含 identity-scoped `load_playback` / play / pause / seek / status / stop；播放缓冲独立核验/解码，不进入工程或分析缓存。`ProjectStore::load` 不访问音频。
- schema v2、migration 和原子保存不变；Decode → Waveform 依赖及缓存键不变，分析保留 provider independence、confidence/provenance 与 cancellation，不持久化成用户事实。
- 统一验证入口 `tools/verify.ps1` 已完成：工具链诊断、MSVC/SDK 自动发现、核心/前端/Tauri 完整检查和 fixture 契约比较；离线运行，失败立即停止并恢复环境，普通验证不改 fixture。CI 调用同一入口，旧脚本保留兼容包装。
- 当前播放变更本地通过 73 项核心测试、21 项前端测试、Rust fmt/Clippy、TypeScript typecheck/Vite build，并完成 Windows GNU target 的 Tauri/CPAL check + Clippy；本轮 Windows 原生 tests/build 待远端 CI。此前基线的 GitHub Actions Ubuntu/macOS Core 与 Windows All/System 已实际通过。

已知限制：无音频拷贝/自动扫描；PCM 上限 64 MiB，无磁盘 artifact 缓存，轨道持有 artifact 不计入 runtime 预算。首个 CPAL 播放切片要求设备原生支持源采样率，尚无设备选择、重采样、loop 或 latency compensation。真实扬声器输出、原生对话框/关闭流程仍未在交互式 Windows 会话验收；隔离工具链布局仍是本机工作区约定。完整 user override 编辑、notes、lyrics、alignment、undo/redo、pitch、autosave 尚未实现。
