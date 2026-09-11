# Phase 0 / 最小项目持久化切片

此页保留该切片完成时的契约与验收记录。当前版本已在 [WAV 切片](phase-0-wav-slice.md)
扩展为 schema v2，默认注册真实 v1→v2 migration。

状态：完成本切片，未达到 M0。依据 SDD §7.1、§28、§34、§53–56；没有核心边界变更。

## 范围与模块

闭环：创建项目 → 保存 `.vocalproj` → 重开 → 重命名 → 再次保存/重开。
项目内容仅身份和名称；不创建空壳音频、track、note 或 analyzer 类型。

- `vocal-domain::identity`：UUID 实体 ID；依赖通用 uuid crate，不依赖 UI、Serde 或存储。
- `vocal-domain::project`：`VocalProject` 权威内存数据。
- `vocal-project::format`：私有 Serde DTO；不向领域泄漏持久化 schema。
- `vocal-project::migration`：显式版本迁移注册/调度。
- `vocal-project::store`：有界 ZIP 读取、验证和原子保存。
- `vocal-project::error`：错误类别、用户消息 key 与底层错误链。

依赖方向为 `vocal-project → vocal-domain`，既有 `vocal-time → vocal-domain` 不变。

## Public interfaces

```rust
EntityId::new() -> EntityId; // UUID v4
// EntityId 实现 FromStr、Display、Copy、Eq、Hash。
VocalProject::new(name: impl Into<String>) -> VocalProject;
VocalProject::from_parts(id: EntityId, name: impl Into<String>) -> VocalProject;
VocalProject::id(&self) -> EntityId;
VocalProject::name(&self) -> &str;
VocalProject::rename(&mut self, name: impl Into<String>);

ProjectStore::default() -> ProjectStore; // 首版无历史格式，不注册虚构迁移
ProjectStore::new(migrations: MigrationRegistry) -> ProjectStore;
ProjectStore::load(&self, path: impl AsRef<Path>) -> Result<VocalProject, ProjectError>;
ProjectStore::save(&self, path: impl AsRef<Path>, project: &VocalProject) -> Result<(), ProjectError>;
MigrationRegistry::register(&mut self, step: Box<dyn ProjectMigration>) -> Result<(), ProjectError>;

trait ProjectMigration: Send + Sync {
    fn from_version(&self) -> u32;
    fn to_version(&self) -> u32;
    fn migrate(&self, raw: serde_json::Value) -> Result<serde_json::Value, ProjectError>;
}
```

`SCHEMA_VERSION=1`；`MAX_JSON_BYTES=1 MiB`；`ProjectError::message_key()` 将 UI 消息与调试原因分开。
`from_version` 保留 SDD 命名，为该方法局部允许 Clippy 的 wrong_self_convention。

## Invariants

- UUID 身份与项目名称无关；重命名、存储、迁移重开不重新生成 ID。
- manifest 与 payload 的 UUID 必须匹配。名称按 UTF-8 原样往返，包括空串与换行。
- 当前 ZIP 恰含两个 JSON 文件，Stored 模式；不把裸 JSON 伪装成工程容器。
- 缺失/未知 manifest 字段、当前 schema 的重复/未知字段、非法 UUID、未知版本、
  缺少 migration、未知包条目、损坏或超限内容都返回错误。
- 文件只在内存中读取，不向文件系统解压包条目。
- 保存先写同目录临时文件、完成 ZIP、`sync_all`、重读并比较 domain，然后原子替换。
  可恢复的写入/验证/替换错误清理临时文件，保留旧目标。
- 加载及迁移无写入副作用；只有显式 save 才写当前 schema。
- migration 只注册唯一相邻升级 `n → n+1`；拒绝倒退、跳级、重复与超当前版本的注册。
  migration 输出仍须通过当前 DTO 和 ID 校验。

## Cache / analysis dependencies

新增：无。失效：无。名称编辑不触发音频、F0、节奏或音乐位置重算。
不持久化 TempoMap 的内存索引。本阶段没有 analyzer/provider/DAG 节点、模型下载或自动分析。

## Overrides / provenance / schema migration

本切片没有自动值与人工值相互覆盖的写入路径，也未实现完整 Revision/Provenance。
包含这些尚不支持内容的工程会拒绝加载，不能作为一个删掉原始分析数据的“成功”项目返回。
未来加入它们时，必须保留全部层次与模型来源，配套真实 schema migration 和 round-trip 测试。

这是首次持久化 schema，不存在已发布的 v0。默认 migration registry 为空。
测试中的 v0→v1 仅用来验证 dispatch、失败传播和源文件不变，明确不是可支持的历史产品格式。
新增持久化字段须编写迁移；breaking 变更须升级 schema。本版严格字段校验意味着新增字段
对旧读取器也不兼容，下一次格式扩展应升级 schema 并保留 v1 fixture。

## Tests / 验收

11 项新增集成测试覆盖：

- 稳定 UUID、Unicode/换行项目名、保存重开与重复保存。
- ZIP manifest/payload 分离及固定的 v1 JSON 兼容 fixture。
- 缺失/错误 schema、未来 schema、非法/不一致 ID、未知字段与包内容。
- 破损、空 ZIP 与超过 entry/container 上限的文件。
- 保存前拒绝超限内容，保存后替换失败清理临时文件，旧内容不变。
- 非法 migration 注册、缺失 migration、成功迁移、失败迁移、未知迁移输出。

2026-09-11，Rust 1.98.1 / Windows GNU：24 项全仓库测试通过，
格式与 Clippy `-D warnings` 通过，两个 public API 示例通过。
复现：`./tools/check-core.ps1`（使用项目隔离工具链与 LLVM dlltool）。
MSVC 本地仍缺 Windows SDK；本轮未声明 MSVC 或远程三平台 CI 已通过。

## 明确的后续工作

本 slice 不包含并发编辑冲突检测、dirty tracking、autosave journal、undo/redo 或音频。
save 是同步显式接口，未来应用服务必须串行调度同一项目的保存，并在线程池执行磁盘 I/O。
原子替换避免半写文件；尚不保证突然断电后的目录元数据持久性，也未做断电故障注入。
底层替换依据 [tempfile persist 契约](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist)。
