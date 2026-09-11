# Phase 0 / 时间系统基础切片

状态：实现。此切片不是整个 Phase 0，也不宣称通过 M0。
设计依据：SDD §6、§7.2–7.4、§11、§50、§54、§56 Task 3。
没有更改核心架构，无需新增偏离 SDD 的 ADR。

## Crates / public interfaces

依赖方向：`vocal-time → vocal-domain → std`。

`vocal-domain::time`：

```rust
const TICKS_PER_QUARTER: i64 = 960;
Seconds::new(f64) -> Result<Seconds, TimeValueError>;
Bpm::new(f64) -> Result<Bpm, TimeValueError>;
Samples::new(i64) -> Samples;
Tick::new(i64) -> Tick;
// 所有值提供 get()；字段私有，Seconds/Bpm 不能绕过验证构造。
```

`vocal-time`：

```rust
enum TempoCurve { Step, Linear }
struct TempoEvent { pub tick: Tick, pub bpm: Bpm, pub curve: TempoCurve }
TempoMap::new(Vec<TempoEvent>) -> Result<TempoMap, TempoError>;
TempoMap::events(&self) -> &[TempoEvent];
TempoMap::tick_to_seconds(&self, Tick) -> Result<Seconds, TempoError>;
TempoMap::seconds_to_tick(&self, Seconds) -> Result<Tick, TempoError>;
```

## Invariants / 数值契约

- Seconds 必须有限，允许负值，负零归一化；BPM 必须有限且严格为正。
- Tick/Samples 保存完整 i64，不混用两种单位。
- TempoMap 非空，首事件位于 tick 0，后续严格递增，不自动排序或去重。
- tick 0 对应 0 秒；负预卷延伸首事件，末事件延伸到之后的时间。
- 事件的 curve 描述从该事件开始的区间；Linear 保留类型，但构造时明确拒绝。
- 秒数为分段积分；按二分查找选择区间。构造 O(n)，每次查询 O(log n)。
- 逆转换舍入到最近整数 tick，精确半 tick 向远离零方向舍入，不代表网格量化。
- f64 换算只接受 `abs(tick) < 2^53`；非有限结果、不可表示的段索引或缩放返回 NumericRange。
  这是表示范围界限，不是极端 BPM/长时间下的亚 tick 精度保证。
  不能把非有限结果以饱和整数形式返回。
- TempoMap 不可原位修改；编辑时以新 events 构造新 map，旧 map 保留。

## Cache / analysis dependencies

新增内存派生索引：`starts_sec` 和 `seconds_per_tick`，由 events 与固定 PPQ 派生。
索引私有，无磁盘缓存、无 artifact hash、无 analysis DAG 节点。
重建 map 时重建索引；无现存分析缓存需要迁移或失效。

未来接入运行时：TempoMap 的有效内容改变应使 musical mapping、quantization、
依赖音乐位置的对齐/词格与视图失效；不能使 decode、waveform、raw F0 失效。
运行时届时须显式声明这些依赖并按 SDD §13 构建内容寻址缓存键。

## Overrides / provenance / schema

此模块只做确定性数学转换，不推断 tempo，不新增自动分析结果。
不读取或写入 Revision/UserOverride，也不创建或丢弃 observation/provenance。
调用方未来从 revision 中解析有效 TempoMap 后再调用此接口。

本切片没有序列化格式或持久化字段，因此没有创建伪 schema v1 或空 migration。
首次引入持久化时必须一起实现版本校验、migration dispatch 和 round-trip；
禁止直接序列化本模块私有缓存。Linear 的类型预留不表示已经支持其工程文件格式。

## Tests

- 无效浮点数、非正 BPM、负零、i64 端点。
- 固定 BPM 的四分音符时长、多个 tempo 段的积分、边界两侧连续性。
- 空 map、非零起点、重复/倒序事件、首/末 Linear 拒绝。
- 负预卷、半 tick 舍入、多个 BPM 的数万次整数往返转换。
- 秒数往返误差、数值溢出、不可表示的 tick/BPM。
- 重建 map 不影响旧 map；public API 示例验证 4800 tick ↔ 3 秒。

2026-09-11 验证结果（Rust 1.98.1）：

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets --offline -- -D warnings`：MSVC 目标通过。
- `cargo +stable-x86_64-pc-windows-gnu test --workspace --offline`：13 项测试全部通过。
- GNU 目标 `tempo_map` 示例通过，输出 4800 tick → 3 秒 → 4800 tick。
- 本机 MSVC 运行测试受到 Windows SDK 缺失的限制（kernel32.lib）；未宣称 MSVC 可运行。
- `.github/workflows/core.yml` 提供三平台检查配置，远程 CI 未执行。
