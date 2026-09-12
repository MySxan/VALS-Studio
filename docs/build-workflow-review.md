# 构建工作流核查与阶段建议

核查日期：2026-09-11。本文保存 Project-backed Desktop Workflow 实施前的审查结论，
属于历史评估与建议；当前状态以 [CURRENT.md](CURRENT.md) 为准，后续目标以 [NEXT.md](NEXT.md) 为准。

## 结论

架构边界和核心测试较可靠，但构建复现、验收入口与真实桌面验证尚未形成稳定闭环。
优先统一验证流程，并接通“保存工程 → 重开 → 恢复波形”；不提前引入具体 AI 模型。

审查覆盖 README、两份构建脚本、CI、SDD、阶段记录、应用服务、ProjectStore 和前端控制器。
审查本身未重跑测试。当时记录为 59 项核心、12 项前端、1 项 MockRuntime IPC 测试通过。

## 效率与质量

| 发现 | 影响 | 优先级 |
| --- | --- | --- |
| 本地桌面构建不运行前端测试、不刷新 Rust DTO fixture，CI 才包含这些步骤 | 本地通过与 CI 通过含义不一致 | 高 |
| 固定 `.tools` 与具体 VS 版本路径，缺统一环境检查及工具链版本固定 | 干净机器复现成本高 | 高 |
| 核查时目录不是 Git 仓库，远程 CI 未运行 | 缺提交基线、补丁历史与远程验收证据 | 高 |
| MockRuntime 与浏览器预览已验证，原生文件选择器未验收 | 不能宣称真实桌面端到端流程通过 | 高 |
| 核心和桌面采用独立 workspace/target，本机使用 GNU/MSVC；CI 仅缓存 npm | 隔离合理，但有重复编译成本 | 中 |
| 每次视口变化立即查询并清空波形，旧请求仅丢弃响应 | 快速操作可能产生多余查询或闪烁，需要测量 | 中 |

应保留的做法：Domain/provider/application/Tauri/DTO 分层；取消竞争和缓存身份测试；
损坏工程与未知字段拒绝；真实 migration；原子保存；有界视口查询；明确区分模拟与原生验收。

当时“不覆盖 overrides”主要来自尚无编辑入口，并不等于完整编辑/保存/重算流程已经验证。
provenance 可在内存 artifact 中追溯，尚未实现分析结果的持久化。
没有连续构建耗时、内存峰值或查询延迟记录，不给出效率改善百分比。

## 原建议的三个可审查补丁

### A：统一构建与验收

- Modules：`tools/`、CI、frontend scripts、构建文档。
- Interfaces：建议 `doctor.ps1`、`verify.ps1 -Scope Core|Desktop|All`，与打包入口分开。
- Invariants：本地与 CI 同一验收定义；失败非零退出；不隐式安装或修改全局环境。
- Cache：只优化按平台、工具链和锁文件隔离的构建缓存，不改变分析缓存。
- Tests：缺失工具诊断、当前 Rust DTO fixture 校验、失败阻断，以及真实桌面导入/取消/错误恢复。
- Compatibility：不触及 overrides、provenance 或 schema。建立 Git 基线后记录冷/增量构建耗时。

### B：保存工程

- Modules：`vocal-app`、`vocal-project`、Tauri、前端保存入口。
- Interfaces：工程创建、Save/Save As、当前工程摘要；revision/generation 只用于应用并发保护。
- Invariants：工程是权威状态；保存串行；原子提交成功才更新路径/dirty；失败保留原文件。
- Cache：保存不分析、不把派生 artifact 写成用户事实。
- Tests：过期身份、失败保存、重复保存、另存为身份保持、对话框取消。
- Compatibility：复用 domain→storage DTO，维持 schema v2 和 migration。

### C：重开并恢复分析

- Modules：`vocal-app` 加载/分析编排，复用 ProjectStore、audio、runtime 和桌面打开入口。
- Interfaces：打开工程、按 project/track 请求分析。
- Invariants：load 不访问音频；离线工程仍可打开；旧分析结果不能发布到新工程；保留所有轨道。
- Cache：先核验源，再按原有 identity 命中或重算；源变化不沿用旧结果。
- Tests：保存重开、源缺失/变化、migration、未来 schema 拒绝、取消和过期结果。
- Compatibility：不丢弃未知数据、不覆盖 user override。

用户随后明确选择实施 B/C 合并的 Project-backed Desktop Workflow；A 的整体环境治理仍待完成。
M0 还需要播放、手工 note/lyrics、保存重开和 JSON 导出，当前只能标记 Phase 0 增量。
