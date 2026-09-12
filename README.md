# VALS Studio

按 [Software Design Document v0.1](docs/architecture/Vocal_Analysis_Software_Design.md)
实施的本地优先人声分析桌面项目。该文档是架构约束；核心边界变更须经用户明确批准 ADR。

当前实现 Project-backed Desktop Workflow，尚未达到 M0。真实状态见 [CURRENT](docs/CURRENT.md)，下一阶段见 [NEXT](docs/NEXT.md)。
Tauri/React 支持 New/Open/Save/Save As/Close、WAV 导入、轨道选择与 Canvas 波形；播放与音乐实体编辑尚未实现。

## 当前切片

- `vocal-domain::time`：有限秒数、正 BPM、采样与 tick 值类型，PPQ=960。
- `vocal-time`：不可变 Step TempoMap，秒/tick 双向换算、负时间预卷和数值错误。
- `vocal-domain::{identity, project}`：稳定 UUID、项目创建与重命名。
- `vocal-audio`：可取消的 WAV 关联导入、容器元数据与 SHA-256 核验，保留原文件。
- `vocal-project`：schema v2 ZIP、真实 v1→v2 migration、原子保存与重开，持久化 source/track。
- `vocal-analysis-api/runtime`：统一 Analyzer、confidence/provenance、依赖校验、有界内存缓存。
- `vocal-audio::DecodeAnalyzer`：可取消、固定快照的原声道/原采样率 PCM 解码（payload 上限 64 MiB）。
- `vocal-dsp`：min/max/RMS 波形 pyramid，按时间范围和像素宽度返回有界视口数据。
- `vocal-app`：以 VocalProject 为权威状态，后台导入完成后原子挂接 source/track，应用层保存重开与源核验；派生分析和 DTO 独立。
- `apps/desktop`：Tauri 2 薄适配层、React/TypeScript、Zustand、Zod、Canvas 波形与 provenance Inspector。
- 没有模型或 Python runtime；依赖与许可证记录见 `THIRD_PARTY.toml` 和 `docs/desktop-dependencies.json`。

详细接口、约束、缓存依赖和验收见 [时间切片](docs/architecture/phase-0-time-slice.md)、
[持久化切片](docs/architecture/phase-0-project-slice.md)、[WAV 切片](docs/architecture/phase-0-wav-slice.md)
与 [schema v2](docs/formats/project-v2.md)。新增阶段记录见
[分析切片](docs/architecture/phase-0-analysis-slice.md)、[视口切片](docs/architecture/phase-0-viewport-slice.md)、[后台会话切片](docs/architecture/phase-0-app-slice.md)。

当前桌面接口、约束和验收见 [Project-backed 切片](docs/architecture/phase-0-project-backed-desktop.md)。Relink 契约见 [源恢复切片](docs/architecture/phase-0-relink-slice.md)。相对路径规则见 [路径切片](docs/architecture/phase-0-relative-source-slice.md)。历史审查见 [构建工作流核查](docs/build-workflow-review.md)。
本机已有原生程序：`apps/desktop/src-tauri/target/debug/vals-desktop.exe`。
运行后 New 或 Open 工程，再 Import WAV；Save/Save As 保存 `.vocalproj`。Close/Open 后选中轨道会核验源并显示波形，源缺失或变化明确显示状态。支持取消、缩放、平移和分析依据查看。选择“重新关联音频”可恢复移动后的同内容 WAV，成功后保存工程；不同内容会被拒绝。工程内音频保存相对关联，目录整体移动后可恢复；Save As 重算路径但不复制音频。
工作区复现构建：`./tools/build-desktop.ps1`。标准环境在 `apps/desktop` 运行 `npm ci`、
`npm test`、`npm run tauri -- dev`。

## 验证

使用 PowerShell 7。统一入口默认离线运行，不安装工具或依赖：

```powershell
./tools/verify.ps1 -DiagnoseOnly
./tools/verify.ps1
```

本地隔离工具链存在时自动使用它；标准环境可指定 `-Environment System`。只检查核心使用 `-Scope Core`，只检查桌面使用 `-Scope Desktop`。旧 `check-core.ps1` / `build-desktop.ps1` 保留为包装入口。

入口统一执行核心 fmt/clippy/tests/五个示例、Rust DTO fixture 比较、前端 tests/build，以及 Tauri fmt/clippy/tests/build。MSVC 与 SDK 通过安装信息和完整版本目录自动发现；失败立即停止并恢复调用环境。工具准备、fixture 显式刷新和 CI 范围见 [验证契约](docs/architecture/phase-0-validation-slice.md)。

本机完整链路通过：70 项核心测试、20 项前端测试、2 项 Tauri IPC 测试、4 项 fixture 契约测试、工具脚本测试和原生构建。正常验证不修改已审查 fixture。GitHub Actions 已通过 Ubuntu/macOS Core 与 Windows All/System；真实原生对话框和窗口关闭仍待交互式 Windows 验收。

可执行文件：`apps/desktop/src-tauri/target/debug/vals-desktop.exe`。源文件状态与下一阶段目标见 [CURRENT](docs/CURRENT.md) 和 [NEXT](docs/NEXT.md)。
