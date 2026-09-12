# Phase 0：Tauri / React 波形预览

> 历史阶段记录。当前已改为 [Project-backed 工作流](phase-0-project-backed-desktop.md)，新增工程操作并移除 session-based IPC。

本切片实现选择 WAV → 后台分析 → Canvas 波形预览。遵守 SDD 的 Tauri 2、React/TypeScript、Zustand、Zod 与 DTO 分离约束；无 ADR 变更。

## Crates / modules

- `apps/desktop/src-tauri`：Tauri commands、AppService state、原生文件选择器插件。
- `src/backend.ts`：唯一生产 IPC adapter，反序列化输出经 Zod 校验。
- `src/model.ts`：独立 frontend DTO schema 与视口接口；不导入 Rust domain internals。
- `src/controller.ts`：Zustand vanilla store、后台任务轮询、当前视口查询和响应竞争处理。
- `src/App.tsx`：工具栏、状态、错误、Inspector；`src/Waveform.tsx`：Canvas 聚合块绘制。
- `src/preview.ts`：仅开发构建中的显式 `?preview=1`，读取真实 Rust 示例 fixture，不进行分析。

桌面采用独立 Cargo workspace/lockfile，以免 GTK/WebView 等平台依赖进入可移植核心检查。
核心通过 path dependencies 复用，crate 边界和根 workspace 不变。桌面依赖清单见
`docs/desktop-dependencies.json`，核心清单仍在 `THIRD_PARTY.toml`。

## Public interfaces

Tauri commands：

| Command | JS 参数 | 返回 |
| --- | --- | --- |
| `start_import` | `{path: string}` | job UUID |
| `job_status` | `{id: string}` | JobStatus |
| `cancel_job` | `{id: string}` | void |
| `current_session` | `{}` | SessionDto 或 null |
| `waveform_slice` | `{sessionId, start, end, width}` | WaveformDto |

时间为秒，width 为整数且范围 1..4096。命令委托现有 AppService；波形查询通过
`spawn_blocking` 离开 WebView 主线程，分析仍由 AppService 后台线程执行。
启动导入命令本身只创建任务。原生文件选择使用 `dialog:allow-open`，没有授予文件写权限。

前端 `Backend` 接口提供 `currentSession / chooseFile / startImport / jobStatus / cancelJob / waveform`；
`createController` 提供 `initialize / importFile / cancel / view / dispose` 和只读订阅 store。

## Invariants

- 前端没有 DSP、PCM 解码或全量音频传输；仅绘制有界 min/max/RMS 聚合块。
- 一个导入任务。200 ms 轮询，不重叠请求；通信失败保持 busy，1 s 后重试，可继续请求取消。
- 取消/失败保留旧会话；只有成功状态才刷新会话并恢复全范围。
- 每次视口查询递增请求序号；只接受最新响应，且 session ID、artifact hash、声道数及输出数量须匹配。
- 初始化响应也受会话代次约束，不能覆盖随后导入的会话。导入错误与查询错误分别保存。
- 后台会话是独立预览；界面没有工程保存、覆盖或编辑操作。
- Canvas 以原始采样率将 frame 换算为秒；完整聚合块按当前视口和声道矩形裁剪。
  不生成缺失采样点。缩放到基础 256-frame 块以内仍显示聚合数据，不伪装成单采样精度。
- 缩放以视口中心为基准；平移按半个视口步长，范围限制在源时长内。
- 页面卸载取消轮询、丢弃在途响应。桌面退出由进程终止结束内存任务；本阶段没有跨重启任务恢复。
- 普通浏览器没有文件导入能力，显示打开桌面应用的提示。开发预览标注 fixture 且禁用导入；
  生产构建不包含 fixture 模块，也不支持预览开关。

## Cache / analysis dependencies

仍为 WAV import → Decode → WaveformPeaks，无新 Analyzer、无算法版本变化或缓存失效。
沿用 AppService 的内存缓存和 provider/version/model/settings/source/dependency identity。
前端仅持有当前 SessionDto 和当前 WaveformDto；查询中释放前一视口结果，旧响应丢弃。
缩放、平移、窗口尺寸改变只查询已有 artifact，不启动分析或新增缓存。

## Overrides / provenance / migration

没有写工程能力，因此不修改 user overrides，schema 保持 v2，v1→v2 migration 未动。
Inspector 展示 confidence kind、score 和解释，Measurement 的 null score 显示“不适用”。
完整 provenance 可展开，包含 source、dependency、settings hashes、provider/model/version、时间和 runtime。
源/分析/模型细节未进入 Canonical Domain Model 的 UI 特有字段。

## 验证

- 59 项核心测试、5 个示例及核心 fmt/Clippy 通过（GNU）。
- 12 项 Vitest：真实 Rust fixture DTO、非法边界、响应乱序、初始化竞争、错误保留、
  视口范围、卸载、成功替换、失败/取消保留、重复导入、轮询恢复、取消文件选择。
  Windows CI 在测试前用当前 Rust 示例重新生成 fixture，以发现跨语言 DTO 漂移。
- TypeScript strict 与 Vite production build 通过；无外部字体/CDN 依赖。
- Tauri MockRuntime IPC 测试通过（MSVC）：经真实 invoke handler 导入 fixture、等待真实后台分析，
  检查 camelCase DTO、有界波形与非法参数/未知任务错误。MockRuntime 不验证 OS 文件选择器。
- 浏览器实测开发预览的 Canvas 绘制、放大、向右平移、恢复全范围、provenance 展开和普通浏览器空状态。
- MSVC 原生 build 和 all-targets Clippy 通过。远程 Windows CI 配置已增加，尚未运行。
- 尚未完成真实原生文件选择器的自动化端到端验收；不把浏览器 fixture 预览当作这项验证。

## 运行与环境

标准 Windows 开发环境需 Node 22.12+、Rust stable MSVC、C++ Build Tools、Windows SDK 和 WebView2。
在 `apps/desktop` 执行 `npm ci`、`npm test`、`npm run tauri -- dev`。
生产前端 + 原生开发二进制：先 `npm run build`，再从根目录执行：

```powershell
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features tauri/custom-protocol --locked
```

本工作区可执行 `./tools/build-desktop.ps1`，使用已安装的隔离 Rust 和 SDK，同时执行构建/检查/IPC 测试。
SDK 来自 Microsoft 官方 NuGet 的 `Microsoft.Windows.SDK.CPP` 与 `.x64` 10.0.28000.2705，
仅解压在 `.tools/windows-sdk`；脚本不下载、不安装系统组件，结束时恢复进程环境变量。
此隔离路径绑定当前机器 VS 18 的工具目录；其他机器使用配置好的 MSVC developer shell。

输出：`apps/desktop/src-tauri/target/debug/vals-desktop.exe`，已嵌入生产前端，运行不需要 Vite。
本阶段关闭 installer bundling，没有签名、安装器或发布操作。

开发预览：`npm run dev` 后访问 `http://127.0.0.1:1420/?preview=1`。
fixture 来自核心 `import_session` 示例的 20 ms 合成 WAV，只用于接口和布局验收。

实现依据：[Tauri commands](https://v2.tauri.app/develop/calling-rust/)、
[Tauri dialog](https://v2.tauri.app/plugin/dialog/)、以及仓库 SDD 第 5、32、33 节。
