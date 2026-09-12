# Project-relative Source 切片

闭环：工程内 WAV → Save 记录相对关联 → 移动整个工程目录 → metadata-only Open → application 核验相对候选 → waveform → Save As 重基准 → 新实例重开。沿用 SDD 的 linked source 字段，不新增 schema 或 ADR。

## Modules / interfaces

新增 `vocal-app::paths` 内部模块，负责工程目录基准、候选解析、核验和保存副本的 URI 重基准。`jobs` 在后台调用它，再进入既有 Decode → Waveform pipeline。`AppService`、IPC 和 frontend Backend 接口不变；TrackDto.sourcePath 是当前已核验位置或待核验的首选候选投影，不是另一份语义源事实。

复用 `AudioUri::Linked`、`VocalProject::set_source_uri`、`WavImporter`、`verify_source` 和 `ProjectStore`。domain 与 persistent schema 本阶段不变。

## Invariants 与路径规则

- Open 只 canonicalize 工程自身路径并 load 元数据，不访问音频。相对路径基准固定为工程文件所在目录，不依赖之后的工作目录。
- 相对路径兼容 `/` 和 `\` 分隔符；domain 既有校验禁止绝对路径、drive prefix、父目录跳转。解析后的真实文件必须仍位于工程目录内，越界 symlink 不被接受。
- 相对候选优先；仅 NotFound 才尝试 absolute fallback。changed、权限或非法文件错误直接报告，不能因 fallback 内容正确就掩盖错误候选。每次分析都核验实际内容，metadata 由既有解码契约继续校验。
- 分析只更新派生状态与实际使用路径，不改 source URI、不标记 dirty、不写工程。工程/source/track 身份、内容 hash/size/metadata 和其他语义字段不变。
- 显式 Relink 验证同内容后，在当前工程目录内生成正确相对路径；目录外使用绝对关联。同 URI 不新增 dirty。替代上一 Relink 切片“总是清空 relative path”的临时策略。

## Save / Save As

保存先克隆完整 domain，重基准副本，再调用既有原子写入。只有写入成功才替换当前 domain / 保存路径并清除 dirty；失败保留原状态与原工程文件。

位置优先采用已成功核验的派生位置；没有时采用旧工程相对目标，否则保留原 absolute fallback。同目录、未核验的普通 Save 原样保留已有两个候选。Save As 不读取音频内容；未核验且包含相对关联时，保留旧相对目标所指的位置。新工程目录能包含该目标则写 `/` 分隔的相对路径；否则写绝对路径、relativePath=null，不能生成 domain 禁止的 `..`。因此未核验的双候选在跨目录 Save As 后可能只保留原相对目标；需要保留 fallback 时先核验轨道或显式 Relink。

不复制/搬运 WAV，不自动扫描文件。无法无损表示为 UTF-8 的源位置拒绝处理，不进行 lossy 持久化。保存目标不可覆盖任一当前相对/绝对音频候选。

## Cache / overrides / migration

没有新 cache key、analysis dependency、provider/model 或算法版本。路径仅用于读取，cache identity 仍基于内容与 metadata / provider / settings / dependencies；cache hit 也需要实际源核验。后台取消、job ID / generation 与 stale response guard 保持，job admission 重新读取当前工程基准以避免并发 Save As 使用旧位置。

schema v2、v1→v2 migration、未知字段拒绝和 ProjectStore load 的 metadata-only 语义不变。分析不写 Raw Observation / Resolved Value / User Override，不覆盖已有 provenance；未来 override 编辑结构仍未实现。

## 验证

Windows 本机：70 项核心测试、20 项 frontend integration tests、2 项真实 Store/runtime 的 Tauri MockRuntime IPC 闭环通过，fmt、Clippy、五个核心示例、TypeScript/Vite 与 MSVC 原生构建通过。测试覆盖目录迁移、新实例重开、fallback、changed 候选不旁路、跨平台分隔符、保存失败原子性、离线 Save As 及防止覆盖相对音频源。Unix symlink 越界测试已加入，尚未在本机执行；真实原生对话框、窗口关闭和远程 CI 仍未验收。
