# Unified Validation 切片

## Modules / public interfaces

本切片只涉及 `tools`、fixture 契约测试与 GitHub Actions，不改业务 crates、domain 或 IPC。

PowerShell 7 入口：

```powershell
./tools/verify.ps1                         # 核心 + 桌面完整链路
./tools/verify.ps1 -DiagnoseOnly            # 只诊断，不编译、不改 fixture
./tools/verify.ps1 -Scope Core              # 核心检查，支持 Windows/macOS/Linux
./tools/verify.ps1 -Environment System      # 使用调用环境的 Cargo/rustup
./tools/verify.ps1 -Scope Desktop -RefreshFixture # 显式刷新后继续所有桌面检查
```

`-Environment Auto` 在存在 `.tools/cargo/bin/cargo.exe` 时使用本地隔离工具链，否则使用 System。Local 当前适用于本 Windows 工作区：核心 GNU、桌面 MSVC，rustfmt 使用默认工具链。System 继承调用方 Cargo/rustup 配置。旧 `check-core.ps1` 和 `build-desktop.ps1` 是同一入口的 Core / Desktop 包装；后者现在也运行前端 tests 和 fixture 校验。

## Invariants / dependencies

验证不安装或下载依赖，不修改机器设置。Cargo 检查使用 `--offline --locked`；环境覆盖仅在进程内，并在成功或异常后精确恢复，包括原本不存在的环境变量。工作目录同样恢复。子进程非零退出立即终止后续阶段。

Windows 通过 vswhere 查找安装位置，在完整的 MSVC toolsets 中按版本选择，不写死 Community/版本路径。SDK 优先使用本地隔离布局，否则读取 Windows Kits 注册表；仅选择 headers / x64 libraries / rc.exe 完整的版本。Core/System 的 MSVC host 也配置原生环境。诊断输出编译器版本、host、Node/npm 与 rc.exe 位置。

需要预先具备 PowerShell 7、Rust/rustfmt/clippy、Node >= 22.14.0、前端依赖，以及 Windows C++ Build Tools / SDK。Local GNU 还要求已准备 llvm-dlltool；缺少任何已检查前提时明确失败。默认不修复或下载这些依赖。标准环境的首次准备由使用者显式执行：

```powershell
rustup component add rustfmt clippy
cargo fetch --locked
cargo fetch --manifest-path apps/desktop/src-tauri/Cargo.toml --locked
# 在 apps/desktop 目录中执行 npm ci
```

所有业务分析缓存键、analysis dependencies、schema/migration、provenance 和未来 user override 边界不变。未引入新的运行时依赖。

## Fixture 契约

每次桌面验证运行真实 Rust `import_session` example，并通过 stdin 交给 `fixture.mjs`。常规 `--check` 不写文件，仅规范化随机实体 UUID、sourcePath、provenance timestamp/runtime 后进行完整结构与值比较；UUID 引用一致性仍检查，新增/删除 DTO 字段、波形内容、confidence、provider 或 dependency hashes 变化均失败。

fixture 是真实 Rust → frontend 的审查契约，不应在 CI 中无条件重写。明确审查变化后才使用 `-RefreshFixture`；刷新输出合法、固定 UUID 与固定 host/path/time，使后续刷新稳定。frontend Zod 和 integration tests 继续验证该 fixture。规范化只作用于开发 fixture，不作用于应用的实际 provenance。

## 检查集合与验证结果

Core：脚本测试 → fmt → clippy → workspace tests → 五个 examples。Desktop：脚本测试 → 当前 Rust fixture 比较 → 四项 Node 契约测试 → 前端 tests/build → Tauri fmt/clippy/tests/build。All 合并二者，脚本测试只跑一次。失败不会打印成功标记；完成后输出产物位置和仍未验收的项目。

CI 显式准备依赖后调用同一入口：Windows 运行 All/System，macOS/Linux 运行 Core/System。本机 All/Local 通过：70 核心测试、20 前端测试、2 Tauri IPC 测试、4 fixture 测试、脚本测试与完整原生构建。脚本测试覆盖 native 非零退出、成功/异常环境恢复、缺依赖、数值版本排序、不完整 MSVC/SDK 排除。已确认正常验证前后 fixture 字节和调用方环境一致。

远程 CI、macOS/Linux 实机运行、原生对话框和窗口关闭验收不由本次结果代替；本入口不会将它们标为通过。
