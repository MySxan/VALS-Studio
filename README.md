# VALS Studio

按 [Software Design Document v0.1](docs/architecture/Vocal_Analysis_Software_Design.md)
实施的本地优先人声分析桌面项目。该文档是架构约束；核心边界变更须经用户明确批准 ADR。

当前完成 Phase 0 的时间系统、项目持久化、关联 WAV 导入、Decode/波形分析、视口查询、后台会话和桌面波形七个切片，尚未达到 M0。
Tauri/React 已接入单轨导入与 Canvas 预览；播放与音乐实体编辑尚未实现。

## 当前切片

- `vocal-domain::time`：有限秒数、正 BPM、采样与 tick 值类型，PPQ=960。
- `vocal-time`：不可变 Step TempoMap，秒/tick 双向换算、负时间预卷和数值错误。
- `vocal-domain::{identity, project}`：稳定 UUID、项目创建与重命名。
- `vocal-audio`：可取消的 WAV 关联导入、容器元数据与 SHA-256 核验，保留原文件。
- `vocal-project`：schema v2 ZIP、真实 v1→v2 migration、原子保存与重开，持久化 source/track。
- `vocal-analysis-api/runtime`：统一 Analyzer、confidence/provenance、依赖校验、有界内存缓存。
- `vocal-audio::DecodeAnalyzer`：可取消、固定快照的原声道/原采样率 PCM 解码（payload 上限 64 MiB）。
- `vocal-dsp`：min/max/RMS 波形 pyramid，按时间范围和像素宽度返回有界视口数据。
- `vocal-app`：后台导入任务、取消、成功时原子替换预览会话、独立的可序列化 DTO。
- `apps/desktop`：Tauri 2 薄适配层、React/TypeScript、Zustand、Zod、Canvas 波形与 provenance Inspector。
- 没有模型或 Python runtime；依赖与许可证记录见 `THIRD_PARTY.toml` 和 `docs/desktop-dependencies.json`。

详细接口、约束、缓存依赖和验收见 [时间切片](docs/architecture/phase-0-time-slice.md)、
[持久化切片](docs/architecture/phase-0-project-slice.md)、[WAV 切片](docs/architecture/phase-0-wav-slice.md)
与 [schema v2](docs/formats/project-v2.md)。新增阶段记录见
[分析切片](docs/architecture/phase-0-analysis-slice.md)、[视口切片](docs/architecture/phase-0-viewport-slice.md)、[后台会话切片](docs/architecture/phase-0-app-slice.md)。

桌面接口、约束和验收见 [桌面波形切片](docs/architecture/phase-0-desktop-slice.md)。
本机已有原生程序：`apps/desktop/src-tauri/target/debug/vals-desktop.exe`。
运行后点击“导入 WAV”；支持取消、缩放、平移、恢复全范围和查看分析依据。
工作区复现构建：`./tools/build-desktop.ps1`。标准环境在 `apps/desktop` 运行 `npm ci`、
`npm test`、`npm run tauri -- dev`。

## 验证

使用 Rust stable 和 Cargo。在 Windows MSVC 上需要 C++ Build Tools 和 Windows SDK，
参见 [Rust 官方安装说明](https://rust-lang.github.io/rustup/installation/windows-msvc.html)。

```sh
cargo fetch --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo test --workspace --offline --locked
cargo run -p vocal-time --example tempo_map --offline --locked
cargo run -p vocal-project --example project_roundtrip --offline --locked
cargo run -p vocal-project --example import_wav --offline --locked
cargo run -p vocal-analysis-runtime --example analyze_waveform --offline --locked
cargo run -p vocal-app --example import_session --offline --locked
```

示例预期输出：tick 4800 → 3 seconds → tick 4800，其中 tick 3840 处从 120 BPM 切换到 60 BPM。

本次机器上为验证下载的工具链位于忽略目录 `.tools/`，未修改系统 PATH。
使用它时须将 `CARGO_HOME` 指向 `.tools/cargo`，`RUSTUP_HOME` 指向 `.tools/rustup`，
并调用 `.tools/cargo/bin/cargo.exe`。

本机验证：Rust 1.98.1，格式与 Clippy 检查通过；59 项测试与五个示例在
`stable-x86_64-pc-windows-gnu` 上通过。桌面另通过 12 项前端测试、1 项 Tauri IPC 测试、
TypeScript/Vite 构建、MSVC Clippy 与原生链接；Windows SDK 已隔离补齐到 `.tools/windows-sdk`。
浏览器 Canvas 交互已验证，真实原生文件选择器尚未自动化验收。核心可使用隔离工具链重跑：

```powershell
./tools/check-core.ps1
```

脚本使用隔离 GNU 工具链的 LLVM dlltool，解决新增 Windows 依赖生成 import library 的需求；
运行后恢复进程环境变量。工具链已在本工作区安装，脚本自身不下载依赖。

已提供 Windows/macOS/Linux 的 GitHub Actions 检查配置；远程 CI 尚未运行。

WAV 示例默认使用合成 fixture，也可在命令末尾加 `-- path/to/vocal.wav`；
它在临时目录执行保存/重开并清理演示工程，原 WAV 不变。

分析示例默认使用 stereo fixture，也接受 `-- path/to/vocal.wav`，演示两个独立节点、
缓存命中、confidence/provenance 和视口查询。当前缓存只在内存，未写入工程；schema 仍为 v2。

下一切片可补齐预览会话到可保存工程的显式流程；开始前重新列出阶段契约。
播放、并行 DAG 调度、磁盘 artifact 与分块长音频仍待后续实现。
