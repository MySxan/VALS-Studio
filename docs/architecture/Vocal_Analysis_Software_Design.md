# Vocal Analysis & Lyric Scansion Studio
## Software Design Document / Architecture Specification v0.1

> **目标读者**：GPT Astra6、后续实现代理、项目维护者、音频/DSP/ML/前端工程师  
> **文档用途**：作为长期工程实现蓝图，而不是一次性原型说明  
> **核心原则**：Local-first、可替换模型、统一领域模型、分析结果可追溯、人工修正不破坏原始分析、跨语言扩展、专业音乐时间轴、低技术债

---

# 0. 执行摘要

本项目拟开发一款面向作曲、作词、Topline、Vocaloid/歌声合成、同人音乐、音游音乐、流行音乐制作与音乐分析场景的桌面应用：用户导入一条或多条人声干声后，软件自动分析并输出结构化的人声旋律与歌词韵律信息，包括但不限于：

- 波形、音量包络、静音与呼吸区间；
- 连续 F0 / pitch curve；
- 音符事件、起止点、音高、时值、滑音、颤音、pitch drift；
- BPM 候选、TempoMap、MeterMap、Beat/Grid 对齐；
- Phrase/句段划分；
- 歌词自动识别；
- 已知歌词的 forced alignment；
- word / syllable / phoneme / mora / tone / stress 等语言信息；
- note ↔ syllable ↔ phoneme ↔ lyric token 对齐；
- 跨语言“词格”/lyric scansion；
- musical stress 与 lexical stress、重拍、长音、句长、melisma 等分析；
- Key、range、phrase density、rhythmic profile 等辅助信息；
- 人工编辑、校正、版本回退；
- MIDI、MusicXML、JSON、CSV、Markdown、PDF 等格式导出。

本软件**不是**“把若干 Python 模型套一个 UI”的程序，也不应将任何具体模型（Whisper、Basic Pitch、CREPE 等）视为系统核心。系统真正稳定且长期积累价值的部分应是：

1. **Canonical Vocal Domain Model**：统一表达音频时间、音乐时间、pitch、note、phrase、lyric、phoneme、alignment、confidence 与 revision；
2. **Analysis DAG**：所有分析器以显式依赖图运行，可缓存、增量重算、取消、重试；
3. **Provider Architecture**：Pitch/ASR/Alignment/Language/Tempo 等均可替换；
4. **Fusion Engine**：将来自不同模型与 DSP 算法的 observation 融合成 resolved musical events；
5. **Editable Analysis**：自动结果与用户 override 分离，任何分析均可恢复、重算与升级；
6. **Language-independent Scansion Model**：词格不是日语专用，也不是英文专用；
7. **Professional Timeline**：同时支持 absolute audio time 与 musical time，并允许 TempoMap 变化；
8. **Local-first Desktop**：默认本地处理、离线可用，云服务仅作为可选 provider。

推荐技术主线：

- Desktop Shell：Tauri 2
- Frontend：React + TypeScript
- UI State：Zustand（或 Redux Toolkit，见 ADR）
- Timeline Rendering：Canvas/WebGL，必要时 PixiJS/WebGPU
- Core / Domain / Project / DSP：Rust
- Decode：Symphonia；必要格式使用 FFmpeg fallback
- Audio I/O：CPAL
- Resample：Rubato
- FFT/DSP：rustfft / realfft + 自研 DSP
- ML Research：Python + PyTorch
- Production ML：ONNX Runtime，经 Rust `ort` binding
- Local ASR：whisper.cpp provider
- Forced Alignment Research：WhisperX / Montreal Forced Aligner provider
- Pitch Baselines：CREPE/torchcrepe、Basic Pitch；后续自研 singing-specific F0 provider
- MIDI：midly / 自建抽象
- Project Container：版本化 `.vocalproj` ZIP/包格式

原则上，Python 不进入最终产品的核心运行时；允许它存在于 `research/`、训练、评测和早期 provider 原型中。

---

# 1. 产品定义

## 1.1 产品工作名称

文档暂以 **Vocal Analysis & Lyric Scansion Studio**（简称 VALS）指代。

名称未来可变；代码命名应避免绑定品牌名称。核心 crate 可以使用通用命名：

- `vocal-domain`
- `vocal-analysis`
- `vocal-audio`
- `vocal-language`
- `vocal-alignment`
- `vocal-project`
- `vocal-export`

## 1.2 核心用户

### A. 作曲 / 编曲者
希望从 topline demo 或 reference vocal 中快速理解：

- 音高走向；
- note rhythm；
- BPM/grid；
- phrase 结构；
- 演唱密度；
- 旋律 range；
- 重音与长音；
- MIDI 化。

### B. 作词者
希望获得：

- 每句可容纳多少 syllable / mora / phonetic slots；
- 哪些位置长音；
- 强拍/弱拍；
- melodic peak；
- melisma；
- phrase boundary；
- 原歌词或 placeholder vocal 的韵律结构。

### C. Vocaloid / 歌声合成用户
希望：

- 干声转 note；
- 歌词与 note 对齐；
- 提取 pitch expression；
- 导出 MIDI/MusicXML/辅助数据；
- 与 SynthV/ACE/DAW 等工作流衔接。

### D. 音乐研究/教育用户
希望查看：

- F0；
- prosody；
- rhythm；
- linguistic-music alignment；
- 可重复、可导出的分析数据。

## 1.3 核心输入

第一阶段：

- 单声道人声干声；
- 双声道但实际为单一 vocal source 的音频；
- WAV / FLAC / MP3 / AAC / M4A / OGG 等常见格式；
- 可选已有歌词文本；
- 可选已知 BPM / time signature / key。

第二阶段：

- 多轨 vocal；
- harmony vocal；
- background vocal；
- 带有限伴奏的人声；
- full mix，经 source separation 后进入分析。

## 1.4 核心输出

### 可视化

- waveform；
- spectrogram；
- pitch curve；
- piano roll；
- beat/bar grid；
- phrase lanes；
- lyric lanes；
- phoneme/syllable lanes；
- confidence overlay；
- alignment links。

### 结构化文档

- Lyric Grid；
- Phrase Sheet；
- Vocal Summary；
- Note Sheet；
- Pronunciation/Prosody Sheet。

### 导出

必须规划：

- MIDI
- MusicXML
- JSON
- CSV
- Markdown
- PDF

可选：

- TextGrid（Praat/MFA 生态）
- SRT/VTT
- UST/USTX
- SynthV 兼容格式（若许可证与格式允许）
- VSQx/VPR 等，仅在格式法律与工程可行性明确后实现

---

# 2. 非目标 / Non-goals

首个稳定版本不应同时承担以下任务：

1. 完整 DAW；
2. 专业音频修音器/Melodyne 替代品；
3. 实时低延迟 Auto-Tune；
4. 全混音自动扒谱；
5. 自动作词大模型；
6. 自动生成歌曲；
7. 多人 speaker diarization；
8. 云端协作套件；
9. VST/AU 插件宿主；
10. 完整乐谱排版软件。

这些可以作为未来能力，但不得污染 v1 的核心领域模型。

---

# 3. 核心设计原则

## 3.1 Domain-first，Model-second

所有外部模型必须输出统一领域数据，不允许 UI 或业务代码依赖：

- Whisper 特有 token；
- Basic Pitch 特有 frame；
- CREPE 特有 activation；
- MFA 特有 TextGrid 结构。

这些内容只能存在于 provider adapter 或 raw observation 中。

## 3.2 Raw Observation 永不丢失

系统至少区分：

- `RawObservation`
- `DerivedAnalysis`
- `ResolvedValue`
- `UserOverride`

示例：

```text
CREPE: pitch = 277.3 Hz @ 4.213s, confidence .92
Onset model: note onset = 4.205s, confidence .81
ASR aligner: syllable onset = 4.188s, confidence .73
Grid: nearest 1/16 = 4.200s

Resolved note onset = 4.203s
User override = 4.195s
```

重分析时不得覆盖用户编辑。

## 3.3 Confidence 是一级数据

任何 AI/启发式分析都不应只输出“答案”。

统一要求：

```ts
interface Confidence {
  score: number;       // normalized 0..1 if meaningful
  kind: ConfidenceKind;
  calibrated?: boolean;
  explanation?: string;
}
```

需要支持：

- uncertain regions；
- alternatives；
- confidence heatmap；
- UI filtering；
- “仅检查低置信度内容”。

## 3.4 用户编辑永远优先

读取最终值的规则：

```text
UserOverride > ManuallyConfirmed > ResolvedAutomatic > RawAutomatic
```

用户可以：

- reset one field；
- reset one object；
- reset one analysis stage；
- recompute while preserving overrides。

## 3.5 Local-first

默认：

- 工程文件在本地；
- 分析在本地；
- 音频不上传；
- 模型下载由用户明确触发；
- 云 provider 作为 optional capability。

## 3.6 可重现分析

每个分析结果必须能够记录：

- analyzer id；
- analyzer version；
- model id/version/hash；
- settings；
- source audio hash；
- dependency hashes；
- timestamp；
- platform/runtime metadata（必要时）。

---

# 4. 总体系统架构

```text
┌─────────────────────────────────────────────────────────┐
│                     Desktop Application                 │
│                                                         │
│ React / TypeScript                                      │
│ - Project Browser                                       │
│ - Waveform / Spectrogram                                │
│ - Piano Roll                                            │
│ - Lyric / Phoneme Editor                                │
│ - Lyric Grid / Scansion                                 │
│ - Inspector                                             │
│ - Export UI                                             │
└────────────────────────────┬────────────────────────────┘
                             │ Tauri commands/events
┌────────────────────────────▼────────────────────────────┐
│                     App Service Layer                   │
│ Rust                                                    │
│ - Commands                                              │
│ - Project transactions                                  │
│ - Undo/redo                                             │
│ - Job scheduler                                         │
│ - Cache manager                                         │
│ - File/model management                                 │
└────────────────────────────┬────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────┐
│                     Analysis Runtime                    │
│                                                        │
│ DAG Scheduler                                          │
│ ├ Decode                                               │
│ ├ Resample                                             │
│ ├ Waveform peaks                                       │
│ ├ Spectrogram                                          │
│ ├ VAD / phrase candidates                              │
│ ├ Pitch / F0                                           │
│ ├ Note segmentation                                    │
│ ├ Onsets                                               │
│ ├ Tempo / Meter inference                              │
│ ├ ASR                                                  │
│ ├ Language analysis                                    │
│ ├ Forced alignment                                     │
│ ├ Note-lyric alignment                                 │
│ ├ Quantization                                         │
│ ├ Key/range/rhythm features                            │
│ └ Fusion                                               │
└────────────────────────────┬────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────┐
│                     Canonical Domain                    │
│                                                        │
│ Audio / Time / Tempo / Meter / Pitch / Note / Phrase   │
│ Lyric / Word / Syllable / Mora / Phoneme / Alignment   │
│ Observation / Confidence / Revision / Override          │
└────────────────────────────┬────────────────────────────┘
                             │
         ┌───────────────────┼────────────────────┐
         ▼                   ▼                    ▼
    Project Store         Exporters            Providers
    .vocalproj            MIDI/PDF/...         ML/DSP/NLP
```

---

# 5. 技术栈决策

## 5.1 Desktop：Tauri 2

推荐而非 Electron，原因：

- Rust 后端与本项目 core 技术方向一致；
- OS WebView，安装体积通常更低；
- message-passing 边界清晰；
- 可将文件系统、模型、线程、DSP、原生库放入 Rust；
- React/TS UI 开发效率高；
- 后续可扩展 updater、deep link、native menu 等。

注意：

- 不允许将所有 core logic 放在 `src-tauri/main.rs`；
- Tauri 只是 adapter；
- domain crate 禁止依赖 Tauri。

## 5.2 Frontend：React + TypeScript

建议：

- React
- TypeScript strict mode
- Vite
- Zustand 作为 UI/session state
- TanStack Query 可用于 async resource state，但不要将本地 domain graph 强行当 server state
- Zod 用于 IPC DTO 边界校验（或生成共享 schema）

大型 timeline 不使用成千上万个 DOM 节点。

## 5.3 Timeline Renderer

推荐分层：

```text
TimelineViewport
 ├ BeatGridLayer
 ├ WaveformLayer
 ├ SpectrogramLayer
 ├ PitchCurveLayer
 ├ NoteLayer
 ├ LyricLayer
 ├ PhonemeLayer
 ├ PhraseLayer
 ├ ConfidenceLayer
 ├ SelectionLayer
 └ InteractionOverlay
```

实现策略：

- React：布局、工具栏、Inspector；
- Canvas/WebGL：高密度时间轴内容；
- 当数据量明显提升时评估 PixiJS 或自研 WebGL renderer；
- WebGPU 作为未来优化，不作为 v1 前置条件。

统一 viewport：

```ts
interface TimelineViewport {
  startSec: number;
  endSec: number;
  pixelsPerSecond: number;
  verticalScale: number;
}
```

## 5.4 Core：Rust

Rust 负责：

- domain；
- persistence；
- analysis graph；
- job scheduler；
- audio decode；
- resample；
- DSP；
- inference adapters；
- export；
- model/cache management。

## 5.5 Python 的定位

只用于：

- ML training；
- notebooks；
- benchmark；
- dataset preparation；
- early algorithm validation；
- 尚未迁移的实验 provider。

默认不在生产安装包中带 Python runtime。

---

# 6. Repository / Workspace 设计

```text
vals/
├── apps/
│   └── desktop/
│       ├── src/                      # React frontend
│       ├── src-tauri/                # thin Tauri adapter
│       └── tests/
│
├── crates/
│   ├── vocal-domain/                 # ZERO UI / ZERO model dependency
│   ├── vocal-time/                   # musical & absolute time
│   ├── vocal-audio/                  # decode, buffers, resample
│   ├── vocal-dsp/                    # FFT, envelopes, utility DSP
│   ├── vocal-analysis-api/           # Analyzer traits/contracts
│   ├── vocal-analysis-runtime/       # DAG, cache, scheduler
│   ├── vocal-pitch/                  # pitch abstraction + DSP helpers
│   ├── vocal-note/                   # note segmentation
│   ├── vocal-tempo/                  # tempo/grid/quantization
│   ├── vocal-asr/                    # transcription abstractions
│   ├── vocal-language/               # linguistic abstractions
│   ├── vocal-alignment/              # audio-text/note-lyric alignment
│   ├── vocal-fusion/                 # resolve observations
│   ├── vocal-project/                # persistence/migrations
│   ├── vocal-export/                 # MIDI/MusicXML/JSON/etc.
│   ├── vocal-model-runtime/           # ONNX / native inference adapters
│   └── vocal-testkit/                 # fixtures/assertions
│
├── providers/
│   ├── pitch-basic-pitch/
│   ├── pitch-crepe/
│   ├── asr-whispercpp/
│   ├── align-whisperx-proto/
│   ├── align-mfa-proto/
│   └── language-*/
│
├── research/
│   ├── notebooks/
│   ├── python/
│   ├── benchmarks/
│   └── datasets/
│
├── models/
│   └── manifests/                    # models themselves may be downloaded
│
├── docs/
│   ├── architecture/
│   ├── adr/
│   ├── formats/
│   └── algorithms/
│
├── fixtures/
│   ├── audio/
│   ├── lyrics/
│   └── expected/
│
└── tools/
```

使用 Cargo workspace。

---

# 7. Canonical Domain Model

这是整个软件最重要的部分。

## 7.1 ID

所有可编辑实体使用稳定 ID：

```rust
struct EntityId(Uuid);
```

不要使用数组 index 作为身份。

## 7.2 时间类型

禁止到处裸用 `f64`。

```rust
struct Seconds(f64);
struct Samples(i64);
struct Tick(i64);
struct Bpm(f64);
```

推荐 internal PPQ：

```text
960 或 1920 ticks per quarter note
```

优点：多数常用 subdivision 可整数表达。

## 7.3 Source Time 与 Musical Time

```rust
struct AudioPosition {
    samples: Samples,
}

struct MusicalPosition {
    tick: Tick,
}
```

转换必须经 `TempoMap`。

## 7.4 TempoMap

```rust
struct TempoMap {
    events: Vec<TempoEvent>,
}

struct TempoEvent {
    tick: Tick,
    bpm: Bpm,
    curve: TempoCurve,
}

enum TempoCurve {
    Step,
    Linear,
}
```

v1 可只支持 Step，但格式预留 Linear。

## 7.5 MeterMap

```rust
struct MeterEvent {
    tick: Tick,
    numerator: u8,
    denominator: u8,
}
```

不要只有 `timeSignature` 单值。

## 7.6 AudioSource

```rust
struct AudioSource {
    id: EntityId,
    uri: AudioUri,
    content_hash: Hash,
    original_sample_rate: u32,
    channels: u16,
    duration: Seconds,
    embedded: bool,
}
```

URI 支持：

- project-relative；
- absolute file；
- embedded asset；
- future remote asset。

## 7.7 VocalTrack

```rust
struct VocalTrack {
    id: EntityId,
    name: String,
    source: EntityId,
    channel_mode: ChannelMode,
    notes: EntityCollection<NoteEvent>,
    phrases: EntityCollection<Phrase>,
    lyric_document: Option<LyricDocument>,
    pitch_track: Option<PitchTrackRef>,
}
```

## 7.8 PitchTrack

Pitch curve 不建议直接 JSON 保存几十万点。

```rust
struct PitchPoint {
    time: Seconds,
    frequency_hz: f32,
    periodicity: f32,
    voiced_probability: f32,
}
```

存储：binary cache，project JSON 仅引用。

未来允许：

- raw pitch；
- smoothed pitch；
- corrected pitch；
- expression pitch。

## 7.9 NoteEvent

```rust
struct NoteEvent {
    id: EntityId,
    audio_span: TimeSpan,
    musical_span: Option<MusicalSpan>,

    pitch: Revision<NotePitch>,
    onset: Revision<Seconds>,
    offset: Revision<Seconds>,

    expression: NoteExpression,
    confidence: Confidence,
    provenance: Provenance,

    lyric_links: Vec<EntityId>,
}
```

`NoteExpression`：

```rust
struct NoteExpression {
    median_pitch_cents: Option<f32>,
    drift_cents: Option<f32>,
    vibrato_rate_hz: Option<f32>,
    vibrato_depth_cents: Option<f32>,
    portamento_in: Option<f32>,
    portamento_out: Option<f32>,
}
```

这些可以逐步实现，字段先预留 optional。

## 7.10 Phrase

```rust
struct Phrase {
    id: EntityId,
    span: Revision<TimeSpan>,
    note_ids: Vec<EntityId>,
    lyric_unit_ids: Vec<EntityId>,
    breath_before: Option<Seconds>,
    breath_after: Option<Seconds>,
    confidence: Confidence,
}
```

## 7.11 LyricDocument

不要直接只存字符串。

```rust
struct LyricDocument {
    source_text: String,
    detected_language: Revision<LanguageTag>,
    tokens: Vec<LyricToken>,
    normalization: TextNormalizationInfo,
}
```

## 7.12 通用语言层级

```text
LyricDocument
  └ Word
      └ Syllable
          ├ Phoneme
          └ LanguageSpecificFeatures
```

注意：并非所有语言都严格符合 Word→Syllable；模型必须允许空层或自定义 grouping。

```rust
struct LyricToken {
    id: EntityId,
    text: String,
    normalized: Option<String>,
    kind: LyricTokenKind,
    span: Revision<Option<TimeSpan>>,
    parent_id: Option<EntityId>,
    features: LinguisticFeatures,
    confidence: Confidence,
}
```

## 7.13 LinguisticFeatures

```rust
struct LinguisticFeatures {
    ipa: Option<String>,
    phonemes: Vec<PhonemeRef>,
    syllable_stress: Option<StressLevel>,
    lexical_tone: Option<ToneValue>,
    mora_count: Option<u16>,
    rhyme_class: Option<String>,
    vowel_nucleus: Option<String>,
    custom: Map<String, JsonValue>,
}
```

语言特性必须扩展，不要在核心 model 里硬编码所有语言。

## 7.14 Alignment

所有对齐都作为实体保存：

```rust
struct AlignmentEdge {
    id: EntityId,
    source: EntityRef,
    target: EntityRef,
    relation: AlignmentRelation,
    weight: f32,
    confidence: Confidence,
    provenance: Provenance,
}
```

支持：

- note → syllable；
- note → phoneme；
- syllable → multiple notes；
- phoneme → note segment；
- word → phrase；
- phrase → bar range。

这样 naturally 支持 melisma 与多 syllable/同音符现象。

---

# 8. Revision / Override 模型

```rust
struct Revision<T> {
    automatic: Option<AnalyzedValue<T>>,
    confirmed: Option<T>,
    user_override: Option<UserEdit<T>>,
}
```

读取：

```rust
fn effective(&self) -> Option<&T>
```

顺序：

1. user override；
2. confirmed；
3. automatic。

用户编辑应记录：

```rust
struct UserEdit<T> {
    value: T,
    timestamp: SystemTime,
    reason: Option<String>,
}
```

未来可加入 history。

---

# 9. Provenance

```rust
struct Provenance {
    analyzer_id: String,
    analyzer_version: String,
    model_id: Option<String>,
    model_version: Option<String>,
    model_hash: Option<Hash>,
    settings_hash: Hash,
    source_hash: Hash,
    dependency_hashes: Vec<Hash>,
}
```

任何自动产生的核心实体都应可回答：

> “它是由什么算出来的？”

---

# 10. Analysis API

## 10.1 Analyzer Trait

```rust
trait Analyzer: Send + Sync {
    fn descriptor(&self) -> AnalyzerDescriptor;
    fn dependencies(&self) -> Vec<AnalysisKind>;
    fn supports(&self, ctx: &AnalysisContext) -> SupportLevel;
    fn run(
        &self,
        ctx: AnalysisContext,
        cancel: CancellationToken,
        progress: ProgressSink,
    ) -> Result<AnalysisArtifact, AnalysisError>;
}
```

## 10.2 Descriptor

```rust
struct AnalyzerDescriptor {
    id: String,
    version: Version,
    kind: AnalysisKind,
    provider: String,
    deterministic: bool,
    hardware: HardwareRequirements,
}
```

## 10.3 AnalysisKind

```rust
enum AnalysisKind {
    Decode,
    CanonicalAudio,
    WaveformPeaks,
    Spectrogram,
    Vad,
    Pitch,
    Onsets,
    Notes,
    Phrases,
    Tempo,
    Meter,
    Quantization,
    Transcription,
    Language,
    Phonemization,
    ForcedAlignment,
    NoteLyricAlignment,
    Key,
    VocalFeatures,
    Fusion,
}
```

---

# 11. Analysis DAG

示例：

```text
Decode
  ↓
CanonicalAudio ────────────────┬────────────────────┐
  ↓                            ↓                    ↓
WaveformPeaks               VAD/Phrase            ASR
  ↓                            ↓                    ↓
UI                          PhraseCandidates    Transcript
                               │                    ↓
CanonicalAudio → Pitch → Notes │              LanguageDetect
      │             ↑          │                    ↓
      └→ Onsets ────┘          │              LinguisticParse
                               │                    ↓
                               └──────────── ForcedAlignment
                                                   ↓
TempoCandidates ← NoteOnsets                      │
      ↓                                            │
TempoMap → Quantization                            │
      └──────────────────────┬─────────────────────┘
                             ↓
                    NoteLyricAlignment
                             ↓
                           Fusion
                             ↓
                    Scansion/Derived Features
```

原则：

- 每个 node 可独立缓存；
- settings 改变只 invalidates downstream；
- 用户手工输入歌词不应触发 Pitch 重算；
- BPM 改变只重算 musical mapping / quantization / views；
- 模型更新时只重算对应分析及下游。

---

# 12. Job System

状态：

```text
Queued
Running
Succeeded
Failed
Cancelled
Stale
```

能力：

- cancellation；
- progress；
- retry；
- priority；
- CPU/GPU resource class；
- concurrent job limits；
- dependency scheduling；
- crash-safe cache writes。

UI 绝不阻塞等待长分析。

Tauri API：

```text
invoke("start_analysis", ...)
→ returns job_id immediately

analysis://progress
analysis://artifact-ready
analysis://failed
```

---

# 13. Cache 设计

Cache key：

```text
hash(
  sourceAudioContentHash,
  analyzerId,
  analyzerVersion,
  modelHash,
  normalizedSettings,
  dependencyArtifactHashes
)
```

不要用：

- file name；
- modified time alone；
- project ID alone。

Cache artifact 要 atomic write：

```text
write temp → fsync if needed → rename
```

Cache 分：

- global model cache；
- global analysis cache；
- project-local artifact cache。

---

# 14. Audio Pipeline

## 14.1 Decode

主路径：Symphonia。

目标：

```text
File → Demux → Decode → PCM
```

推荐 canonical memory representation：

```rust
struct AudioBuffer {
    sample_rate: u32,
    channels: u16,
    layout: ChannelLayout,
    samples: Arc<[f32]>,
}
```

对长文件不要强制一次性整段常驻内存；允许：

- mmap；
- chunked decoded cache；
- tiled waveform cache。

## 14.2 FFmpeg fallback

Symphonia 不能可靠覆盖的 edge cases，可提供可选 FFmpeg backend。

但：

- license 与 binary distribution 必须单独审查；
- 不让 FFmpeg 数据结构进入 domain。

## 14.3 Resample

使用 Rubato 等成熟实现。

Canonical source 不应 destructive 转成 16 kHz。

保存 original-rate PCM 或 source reference；不同 analyzer 请求自己的 working rate：

```text
ASR      16 kHz mono
Pitch    provider-defined
Display  original / convenient
```

## 14.4 Mono strategy

输入 stereo：

- default equal-power mono mix；
- user 可选择 L/R/Mid；
- analyzer setting 必须进 cache key。

## 14.5 Playback

CPAL 提供低层跨平台 audio I/O。

需要独立 `PlaybackEngine`：

- play/pause/seek；
- loop region；
- latency compensation；
- cursor synchronization；
- audio device selection。

实时线程禁止：

- 分配大量内存；
- logging；
- mutex 长锁；
- ML inference。

---

# 15. Waveform 与 Spectrogram

## 15.1 Waveform peaks pyramid

导入音频后生成多级 peak cache：

```text
Level 0: every 256 samples
Level 1: every 512
Level 2: every 1024
...
```

每 block 保存 min/max/RMS。

缩放时取合适 level，而不是每 frame 扫全音频。

## 15.2 Spectrogram

v1 可：

- STFT；
- Hann；
- configurable FFT size；
- mel optional。

缓存 tile，不保存一整张超大 bitmap。

---

# 16. Pitch / F0 架构

## 16.1 抽象接口

```rust
trait PitchProvider {
    fn estimate(&self, audio: AudioView, config: PitchConfig)
        -> Result<PitchObservationSeries>;
}
```

统一输出：

```rust
struct PitchObservation {
    time: Seconds,
    frequency_hz: f32,
    confidence: f32,
    voiced_probability: Option<f32>,
}
```

## 16.2 Provider 策略

### Baseline A：CREPE/torchcrepe

优点：

- 单音 pitch tracking 常用 benchmark；
- 输出 periodicity/confidence；
- 适合作为研究 baseline。

缺点：

- PyTorch 版本不适合作为最终桌面 runtime；
- singing 特殊区域仍需后处理；
- 需要处理 octave error / silence。

### Baseline B：Basic Pitch

优点：

- audio-to-MIDI；
- pitch bend；
- 单一 source 使用较合适；
- 有 ONNX runtime 路径。

缺点：

- 它是通用 AMT，不是专门 vocal F0；
- note segmentation 与本项目需求并不完全一致。

因此建议：

- 作为 note/transcription baseline provider；
- 不把其 note 数据直接当 canonical truth。

### Future：自研 Singing Pitch Provider

未来可训练：

- vocal-specific F0；
- voiced/unvoiced；
- breath/noise；
- vibrato aware；
- onset-aware。

通过 ONNX 进入 production。

## 16.3 Pitch 后处理

独立模块，不绑模型：

- median filter；
- confidence threshold；
- hysteresis voiced mask；
- octave-jump correction；
- short-gap interpolation；
- cents conversion；
- smoothing for display；
- preserve raw curve。

---

# 17. Note Segmentation

F0 ≠ note。

Note segmentation 应独立于 pitch provider。

输入：

- pitch curve；
- onset candidates；
- VAD；
- amplitude envelope；
- optionally ASR phoneme boundaries。

考虑因素：

- pitch stability；
- pitch jump；
- onset energy；
- syllable onset；
- short note suppression；
- portamento；
- vibrato 不应被切成多个 note。

输出 NoteCandidate，并保留 segmentation confidence。

可以先 heuristic：

1. voiced region；
2. pitch clustering；
3. onset snapping；
4. minimum duration；
5. merge by pitch continuity。

未来可换 ML onset/note segmentation provider。

---

# 18. Tempo / Beat / Meter

这是 solo vocal 场景中较难的一块。

## 18.1 不定义“唯一 BPM”

内部输出 TempoHypothesis：

```rust
struct TempoHypothesis {
    bpm: f64,
    phase: Seconds,
    score: f32,
    meter_candidates: Vec<MeterHypothesis>,
    quantization_error: f32,
}
```

保留 half/double tempo alternatives。

## 18.2 数据来源

- note onset IOI；
- syllable onset；
- phrase boundaries；
- energy accent；
- autocorrelation；
- tempo priors；
- candidate quantization loss。

## 18.3 现有工具

研究 benchmark 可使用：

- Essentia；
- librosa；
- aubio；
- madmom（若项目状态/许可证合适）。

最终产品可逐步自己掌控 solo-vocal tempo scoring。

## 18.4 Quantization objective

对候选 tempo/phase/subdivision 计算：

```text
Loss =
  onset_distance
+ duration_distance
+ complexity_penalty
+ triplet_penalty(if unsupported by evidence)
+ tempo_prior
+ phrase/bar consistency
```

不要只最小化 onset 误差，否则极高 BPM 会过拟合。

## 18.5 用户确认

UI 应允许：

- 选择候选 BPM；
- tap tempo；
- drag beat grid；
- set downbeat；
- half/double；
- manual tempo map。

---

# 19. ASR / Singing Transcription

## 19.1 Provider Interface

```rust
trait TranscriptionProvider {
    fn transcribe(
        &self,
        audio: AudioView,
        language_hint: Option<LanguageTag>,
        config: TranscriptionConfig,
    ) -> Result<TranscriptObservation>;
}
```

## 19.2 whisper.cpp

适合作为本地默认 ASR provider：

- native C/C++；
- 多平台；
- CPU 与多类硬件后端；
- 可量化；
- 不需要 Python runtime。

但重要限制：

**Whisper 是 speech ASR，不等于 singing ASR。**

UI 必须明确其结果“可编辑”，architecture 必须允许替换。

## 19.3 ASR 输出不要成为最终歌词

应生成：

```rust
TranscriptObservation {
  segments,
  tokens,
  language,
  alternatives,
  confidence,
}
```

随后：

- normalize；
- linguistic parse；
- align；
- user edit。

---

# 20. Forced Alignment

当用户提供歌词时，优先走 forced alignment，而不是让 ASR 猜内容。

## 20.1 Research Providers

### WhisperX

适合研究 word-level/phoneme-based forced alignment 流程。

### Montreal Forced Aligner

成熟的 forced alignment 工具链，可作为 benchmark，特别适合验证英文、日文、普通话等已有模型场景。

## 20.2 产品架构

```rust
trait ForcedAlignmentProvider {
    fn align(
        &self,
        audio: AudioView,
        transcript: &NormalizedLyricDocument,
        language: LanguageTag,
    ) -> Result<AlignmentObservation>;
}
```

不要让 MFA/WhisperX 的 TextGrid/wav2vec2 数据结构泄漏出去。

## 20.3 Singing-specific 差异

需考虑：

- sustained vowels；
- consonant anticipations；
- melisma；
- inserted vocalization；
- omitted syllables；
- breath/noise；
- altered pronunciation。

因此 forced alignment 只能提供 observation，最终还需 musical alignment fusion。

---

# 21. Language Provider Architecture

```rust
trait LanguageProvider: Send + Sync {
    fn language_support(&self) -> Vec<LanguageTag>;
    fn normalize(&self, text: &str) -> NormalizedText;
    fn tokenize(&self, text: &NormalizedText) -> Vec<LanguageToken>;
    fn syllabify(&self, token: &LanguageToken) -> Vec<Syllable>;
    fn phonemize(&self, text: &NormalizedText) -> PhonemeSequence;
    fn prosody(&self, text: &NormalizedText) -> LinguisticProsody;
}
```

## 21.1 Generic Provider

最低能力：

- Unicode tokenization；
- optional IPA via eSpeak NG/other backend；
- generic syllable slots。

## 21.2 English

关注：

- word；
- syllable；
- lexical stress；
- phoneme；
- rhyme nucleus。

可研究：

- CMUdict；
- eSpeak NG；
- grapheme-to-phoneme model。

## 21.3 Japanese

关注：

- normalization；
- reading；
- mora；
- long vowels；
- 促音；
- 拗音；
- phoneme。

可研究：

- SudachiPy / Sudachi dictionary；
- MeCab/fugashi；
- OpenJTalk dictionary/G2P。

## 21.4 Mandarin Chinese

关注：

- character/word segmentation；
- pinyin；
- tone；
- erhua；
- tone sandhi（是否采用 surface tone 需区分）；
- phoneme/initial/final。

可研究：

- pypinyin；
- jieba / pkuseg / modern NLP tokenizer；
- Mandarin G2P。

## 21.5 Korean

关注：

- Hangul syllable block；
- jamo；
- pronunciation changes；
- batchim；
- G2P。

可研究 g2pK 等工具。

## 21.6 Provider Manifest

Language provider 需声明：

```text
supports_tokenization
supports_syllabification
supports_phonemization
supports_stress
supports_tone
supports_mora
```

UI 根据能力显示功能，而不是假设所有语言一致。

---

# 22. Note ↔ Lyric Alignment

这是产品核心之一。

## 22.1 目标

建立：

```text
phoneme ↔ time
syllable ↔ time
note ↔ time
```

然后求：

```text
note ↔ syllable / phoneme
```

## 22.2 支持关系

- 1 syllable : 1 note
- 1 syllable : N notes (melisma)
- N syllables : 1 note
- consonant before note onset
- vowel sustained through multiple notes
- non-lexical vocalization

## 22.3 Alignment score

候选 edge 可考虑：

- temporal overlap；
- onset proximity；
- vowel region overlap；
- note duration；
- forced alignment confidence；
- ASR confidence；
- language constraints；
- continuity penalty。

可实现 dynamic programming / Viterbi。

不要用简单 nearest-neighbor。

---

# 23. Fusion Engine

Fusion Engine 将 observation 转成 resolved domain state。

## 23.1 输入

- raw F0；
- onset observations；
- note candidates；
- transcript；
- phoneme timings；
- phrase candidates；
- tempo hypotheses；
- quantization；
- user hints。

## 23.2 输出

- resolved notes；
- resolved phrase spans；
- resolved lyric units；
- resolved alignments；
- confidence；
- alternatives。

## 23.3 第一版策略

先做可解释 heuristic fusion：

```text
weighted observations + constraints
```

不要过早训练 end-to-end 模型。

原因：

- 数据难收；
- debug 困难；
- 用户需要可解释修改；
- provider 还会变化。

未来可将 fusion 替换为 probabilistic graphical model 或 learned ranker。

---

# 24. Scansion / “词格”模型

**词格不是 domain source-of-truth，而是 derived view。**

输入：

- note；
- beat/grid；
- lyric unit；
- phoneme；
- stress/tone/mora；
- phrase。

输出多个 renderer。

## 24.1 Universal Lyric Grid

```text
Bar 12
Beat      1      &      2      &      3      &      4      &
Note      C5     D5     E5────         G5     F5────
Lyric     re     mem    ber            me
Stress           ●
Slot      S      S      L             S      L
```

## 24.2 Lyric Slot

```rust
struct LyricSlot {
    span: MusicalSpan,
    duration_class: DurationClass,
    accent: AccentStrength,
    melodic_peak: bool,
    sustain: bool,
    note_ids: Vec<EntityId>,
    lyric_ids: Vec<EntityId>,
}
```

## 24.3 Language overlays

English：stress  
Japanese：mora  
Mandarin：tone  
Korean：syllable block/phoneme  
Generic：syllable/phoneme

## 24.4 Derived warnings

未来可提供可选提示：

- lexical stress vs musical stress conflict；
- excessively compressed syllables；
- long consonant on sustained note；
- tone/melody conflict；
- lyric density anomalies。

这些必须是辅助提示，不应声称语言规则绝对决定艺术选择。

---

# 25. Key / Scale / Range

对于 monophonic vocal，可从 resolved notes 估计 key candidates。

输出候选，不绝对化：

```text
F# minor .72
A major .41
D major .18
```

支持用户直接设置 key。

Vocal range：

- raw min/max；
- robust 5–95 percentile；
- tessitura；
- phrase range。

---

# 26. UI / UX 信息架构

## 26.1 Workspace

建议：

```text
┌ Menu / Transport ─────────────────────────────┐
│                                              │
├ Track/Mode │           Timeline              │ Inspector
│            │                                 │
│            │                                 │
├────────────┴─────────────────────────────────┤
│            Lyric / Analysis Panel            │
└──────────────────────────────────────────────┘
```

## 26.2 主要模式

- Overview
- Pitch
- Notes
- Lyrics
- Alignment
- Scansion
- Analysis Diagnostics

底层数据同一套，不生成互相独立的副本。

## 26.3 Confidence UX

建议：

- 普通：无特殊标记；
- medium confidence：细 dotted underline；
- low：warning outline；
- unknown：灰色；
- 用户确认：check indicator。

提供：

> Review uncertain results

快速跳转到低 confidence 区域。

## 26.4 编辑行为

支持：

- split note；
- merge notes；
- drag onset/offset；
- pitch snap；
- edit lyric；
- split/merge syllable；
- relink note↔syllable；
- set phrase；
- set downbeat；
- choose tempo hypothesis。

所有操作进入 command/undo 系统。

---

# 27. Undo / Redo

不要使用“保存整个 project snapshot”做每次 undo。

Command pattern：

```rust
trait EditCommand {
    fn apply(&self, project: &mut Project);
    fn revert(&self, project: &mut Project);
}
```

也可采用 event-sourced edit log，但 v1 不必过度复杂。

必须支持跨多个实体的 transaction：

例如 split note 会同时修改：

- note；
- lyric alignment；
- phrase membership。

必须原子 undo。

---

# 28. Project 文件格式

扩展名：

```text
.vocalproj
```

推荐 ZIP container：

```text
project.vocalproj
├ manifest.json
├ project.json
├ source/
│  └ optional embedded audio
├ analysis/
│  ├ pitch/*.bin
│  ├ waveform/*.bin
│  ├ spectrogram/*.bin
│  └ observations/*.json
├ assets/
└ revisions/
```

## 28.1 manifest.json

```json
{
  "format": "vocal-project",
  "schemaVersion": 1,
  "createdWith": "0.1.0",
  "projectId": "..."
}
```

## 28.2 Source Audio Strategy

模式：

- linked；
- embedded；
- copy-on-save-as。

Linked 音频记录：

- relative path；
- absolute fallback；
- hash；
- size。

找不到时通过 hash/relink 恢复。

## 28.3 Migration

从第一版开始：

```rust
trait ProjectMigration {
    fn from_version(&self) -> u32;
    fn to_version(&self) -> u32;
    fn migrate(&self, raw: JsonValue) -> Result<JsonValue>;
}
```

禁止“等以后再加 schema version”。

---

# 29. Model Management

不要把所有模型直接打进安装包。

## 29.1 Model Manifest

```json
{
  "id": "pitch-model-x",
  "version": "1.2.0",
  "runtime": "onnx",
  "sha256": "...",
  "size": 123456789,
  "license": "...",
  "source": "...",
  "input": {
    "sampleRate": 16000
  }
}
```

## 29.2 Model Store

```text
~/.vals/models/
```

支持：

- version coexistence；
- checksum；
- disk usage；
- delete；
- update；
- license info。

## 29.3 Hardware Provider

ONNX Runtime EP 检测：

- CPU；
- CUDA；
- DirectML；
- CoreML；
- others where appropriate。

不要硬编码 GPU 必须存在。

---

# 30. ONNX Runtime 策略

训练：PyTorch。

部署优先：

```text
PyTorch → ONNX → ORT
```

优点：

- training/runtime 解耦；
- Rust 可调用；
- execution provider 可扩展；
- 更容易统一模型管理。

限制：

- 部分模型有 unsupported ops；
- dynamic axes 与 preprocessing 要谨慎；
- performance 必须 benchmark；
- whisper.cpp 类成熟 native runtime 不必为了“统一”硬转 ONNX。

因此：

> ONNX 是 preferred runtime，不是宗教。

---

# 31. 插件 / Provider 扩展机制

第一阶段采用 **compile-time/internal provider interface**，不要一开始开放不受信任 dynamic plugin ABI。

原因：

- Rust stable ABI 不简单；
- security；
- model/runtime dependency；
- cross-platform packaging。

第二阶段可引入：

### Option A：Process Plugin Protocol

外部 provider 独立进程：

```text
stdin/stdout JSON-RPC / protobuf
shared temp files / memory map
```

优点：

- 语言无关；
- 崩溃隔离；
- Python research provider 可接入；
- ABI 稳定。

### Option B：WASM Plugin

适合轻量无 native dependency 的 linguistic/transform plugin。

不建议用于大型 GPU 模型起步。

---

# 32. IPC 设计

Frontend 与 Rust 之间使用 coarse-grained commands。

错误示例：

```text
getPitchPoint(1)
getPitchPoint(2)
...
```

正确：

```text
get_pitch_window(track, start, end, resolution)
```

对于大型 binary：

- 不通过巨大 JSON；
- 使用 binary file/mmap；
- 或 Tauri channel/stream 能力；
- 前端请求 viewport downsample。

---

# 33. DTO 与 Domain 分离

Rust domain 不应因为 React 改字段。

```text
Domain Entity
   ↓ mapper
IPC DTO
   ↓
TypeScript
```

可用 schema/codegen 保证同步，但不直接共享所有 internal structs。

---

# 34. Persistence Transaction

编辑过程使用：

- in-memory authoritative project model；
- dirty tracking；
- autosave journal；
- explicit save。

防崩溃：

```text
project.vocalproj
project.vocalproj.autosave
```

保存：

```text
write temp
validate
rename atomic
```

---

# 35. Error Model

错误分类：

```rust
enum AppError {
    AudioDecode,
    AudioDevice,
    ModelUnavailable,
    ModelCorrupt,
    Inference,
    InvalidProject,
    Migration,
    AnalysisCancelled,
    UnsupportedFormat,
    OutOfMemory,
    Export,
    Internal,
}
```

UI 文案与 debug detail 分开。

错误必须包含：

- user-safe message key；
- internal cause chain；
- optional recovery action。

---

# 36. Logging / Diagnostics

使用 structured logging（Rust 可采用 tracing 生态）。

等级：

- ERROR
- WARN
- INFO
- DEBUG
- TRACE

默认日志不得包含完整歌词/用户音频内容。

Diagnostic export 可生成：

- app version；
- OS；
- model versions；
- job logs；
- hardware info；
- sanitized project metadata。

---

# 37. Privacy / Security

Local-first 默认：

- 不上传音频；
- 不上传歌词；
- telemetry 默认关闭或严格匿名且显式同意；
- 云 provider 使用前显示数据去向。

模型下载：

- TLS；
- SHA256；
- manifest signing future；
- license presentation。

项目文件解压必须防 Zip Slip。

模型文件不得直接作为可执行代码加载，除非 runtime 需要且已校验。

---

# 38. Export Architecture

```rust
trait Exporter {
    fn descriptor(&self) -> ExporterDescriptor;
    fn export(&self, project: &Project, options: ExportOptions)
        -> Result<ExportArtifact>;
}
```

## 38.1 MIDI

输出：

- resolved notes；
- tempo map；
- meter map；
- lyric meta events optional；
- pitch bend optional。

需要定义 bend range 与 curve simplification。

## 38.2 MusicXML

输出：

- quantized notes；
- lyrics；
- measures；
- ties；
- rests。

MusicXML 比 MIDI 更依赖量化正确性，因此不要直接从 raw time 导出。

## 38.3 JSON

定义稳定 public schema，与 internal project schema 分开。

原因：内部 schema 可能演进更快。

## 38.4 PDF / Markdown

通过独立 Report Model：

```text
Project → ReportBuilder → Report AST → HTML/Markdown/PDF
```

不要让 PDF exporter 自己读所有 domain 细节。

---

# 39. Scansion Report 示例字段

```text
Project
Track
Detected language
BPM hypothesis
Meter
Key candidates
Range
Phrase count

Phrase N
- absolute time
- bar/beat range
- note count
- lyric unit count
- syllable count
- rhythmic slot pattern
- accent positions
- melodic peak
- longest sustain
- melisma locations
- confidence warnings
```

---

# 40. Testing Strategy

## 40.1 Unit Tests

必须覆盖：

- time conversion；
- tempo map；
- quantization；
- alignment；
- project migration；
- revision precedence；
- cache key；
- text normalization。

## 40.2 Golden Tests

固定小音频 + expected artifacts。

例如：

```text
fixtures/audio/sine_c4.wav
fixtures/audio/vibrato_a4.wav
fixtures/audio/simple_scale.wav
fixtures/audio/test_vocal_en.wav
...
```

验证：

- pitch median；
- note count；
- timings；
- export snapshot。

## 40.3 Property-based Tests

特别适合：

- sec ↔ tick round-trip；
- tempo map；
- project serialization；
- split/merge note invariants。

Rust 可采用 proptest。

## 40.4 Integration Tests

测试完整 DAG：

```text
audio → analysis → edit → save → reopen → export
```

## 40.5 UI Tests

重点：

- timeline coordinate mapping；
- selection；
- drag edit；
- undo/redo；
- large project virtualization。

## 40.6 ML Benchmark

每个 provider 必须有版本化 benchmark，而非“听起来不错”。

Pitch：

- raw pitch accuracy；
- raw chroma accuracy；
- voiced recall/false alarm。

Notes：

- onset precision/recall/F1；
- offset；
- note-with-offset F1。

ASR：

- WER/CER；
- language-specific metric。

Alignment：

- median boundary error；
- 90th percentile；
- syllable association accuracy。

Tempo：

- correct / half / double aware accuracy；
- onset quantization residual。

---

# 41. Dataset Strategy

不要假设一个 dataset 可以覆盖全部。

需要：

1. synthetic controlled data；
2. speech datasets；
3. singing datasets；
4. internally labeled difficult vocal clips；
5. multilingual samples。

内部标注格式直接对齐 canonical evaluation schema。

必须记录 dataset license，避免后期商业化问题。

---

# 42. Performance Targets

这些是目标，不是首版保证。

典型 3 分钟 mono vocal：

- waveform ready：< 1s–2s preferred；
- pitch：real-time factor < 0.5 on normal desktop preferred；
- ASR：由模型/hardware 决定，必须后台运行；
- scrolling：60 fps target；
- playback UI cursor：稳定；
- project save：常规项目 < 500ms preferred（binary cache 不全部重写）。

UI 渲染性能和分析性能分离。

---

# 43. Memory Strategy

禁止长期在前端保存完整 raw PCM。

Rust 持有音频 / cache，前端拿：

- viewport waveform peaks；
- viewport pitch points；
- visible entities。

长音频使用 chunk/tile。

模型 inference 使用独立 buffer pool。

---

# 44. Concurrency

分资源池：

```text
IO Pool
CPU DSP Pool
Inference Pool
GPU-exclusive Queue(optional)
Realtime Audio Thread
```

避免多个大型模型同时抢 GPU 导致 OOM。

scheduler 读取 provider resource hint：

```rust
HardwareRequirements {
    cpu_threads,
    gpu_memory_estimate,
    exclusive_gpu,
}
```

---

# 45. Cross-platform

首要：

- Windows x86_64
- macOS arm64

其次：

- macOS x86_64（视市场）
- Linux x86_64

必须在 CI 从早期持续构建，不要最后才移植。

Windows：

- WASAPI；
- DirectML/CUDA optional。

macOS：

- CoreAudio；
- Metal/CoreML provider where applicable。

---

# 46. Licensing Strategy

每引入一个库/模型，记录：

- source；
- license；
- redistribution；
- model weights license；
- attribution；
- commercial-use constraints。

创建：

```text
THIRD_PARTY.toml
```

不要只看代码 license，模型权重许可可能不同。

特别对：

- FFmpeg build flags；
- pretrained models；
- dictionaries；
- phonetic lexicons；
- datasets。

---

# 47. 推荐开源组件角色表

| 组件 | 推荐角色 | 是否核心依赖 | 备注 |
|---|---|---:|---|
| Tauri 2 | Desktop shell | 是 | 仅应用适配层 |
| React/TS | UI | 是 | 不含 DSP |
| Symphonia | decode | 是/优先 | Rust-native |
| CPAL | playback/capture | 是/优先 | 低层 audio I/O |
| Rubato | resampling | 是/优先 | 避免自造重采样器 |
| rustfft/realfft | DSP | 是/优先 | FFT |
| ONNX Runtime + ort | ML runtime | preferred | 非唯一 runtime |
| whisper.cpp | local ASR provider | provider | 可替换 |
| Basic Pitch | AMT baseline | provider | 不作为 domain truth |
| CREPE/torchcrepe | pitch baseline/research | research/provider | Python 版不进 core |
| WhisperX | forced alignment benchmark | research/provider | Python |
| MFA | forced alignment benchmark/provider | research | 工具链较重 |
| eSpeak NG | generic phonemization candidate | provider | 检查语言质量 |
| SudachiPy/MeCab/OpenJTalk | Japanese analysis research | provider | 不写死 |
| pypinyin | Mandarin helper | provider | 仅是部分语言能力 |

---

# 48. 不推荐的架构

## 48.1 React + FastAPI localhost + 一堆 Python

适合 prototype，不适合长期主架构。

问题：

- runtime packaging；
- virtualenv；
- dependency conflicts；
- child process lifecycle；
- anti-virus；
- GPU dependencies；
- installer size；
- cross-platform complexity。

允许：研究阶段 provider prototype。

## 48.2 Electron 中所有分析都写 JS

问题：

- DSP/ML native integration 不自然；
- audio pipeline 更复杂；
- 内存；
- runtime 打包。

## 48.3 一个 `analyzeAudio()` 大函数

不可接受。

会导致：

- 无增量重算；
- 无 cache；
- 无 provider replacement；
- 无 debug provenance；
- 后期完全耦合。

## 48.4 把“词格”存成最终字符串

不可接受。

词格必须从 domain events 派生。

## 48.5 把 BPM 存成全局单 float

最低也应 `TempoMap`。

## 48.6 自动分析直接覆盖用户编辑

不可接受。

---

# 49. API / Contract 示例

## 49.1 Rust Analyzer Artifact

```rust
struct AnalysisArtifact {
    kind: AnalysisKind,
    artifact_hash: Hash,
    created_at: SystemTime,
    provenance: Provenance,
    payload: ArtifactPayload,
}
```

## 49.2 Frontend note DTO

```ts
interface NoteDTO {
  id: string;
  startSec: number;
  endSec: number;
  startTick?: number;
  endTick?: number;
  midi: number;
  cents: number;
  confidence: number;
  source: 'auto' | 'confirmed' | 'user';
  lyricIds: string[];
}
```

## 49.3 Timeline query

```text
getTimelineSlice({
  trackId,
  startSec,
  endSec,
  pixelWidth,
  layers: ["waveform", "pitch", "notes", "lyrics"]
})
```

backend 根据 resolution downsample。

---

# 50. 首版工程实施顺序

## Phase 0 — Architecture Skeleton

目标：没有 AI 也能成为正确的软件骨架。

实现：

- Cargo workspace；
- Tauri + React；
- `vocal-domain`；
- typed time；
- project load/save；
- schema version；
- import WAV；
- waveform；
- playback；
- timeline viewport；
- manually create/edit notes；
- manually create lyric tokens；
- alignment UI；
- basic scansion view；
- undo/redo。

**验收：手工输入所有分析信息时，软件工作流已经成立。**

## Phase 1 — Audio Foundation

- Symphonia；
- resampling；
- waveform cache pyramid；
- spectrogram；
- robust playback；
- non-WAV formats。

## Phase 2 — Pitch

- provider interface；
- first pitch provider；
- raw pitch visualization；
- confidence；
- smoothing；
- cache/provenance。

## Phase 3 — Notes

- onset analysis；
- heuristic segmentation；
- split/merge UI；
- pitch expression stats；
- MIDI export。

## Phase 4 — Tempo / Grid

- tempo hypotheses；
- beat phase；
- quantization；
- manual downbeat；
- tempo alternatives；
- tick mapping。

## Phase 5 — Lyrics

- whisper.cpp provider；
- text editing；
- language detection；
- generic tokenizer；
- word timing。

## Phase 6 — Linguistic Providers

优先做 2–3 种语言验证 abstraction，而不是一口气做十种：

- English；
- Japanese；
- Mandarin Chinese。

重点是验证：

- stress；
- mora；
- tone。

## Phase 7 — Alignment

- forced alignment prototype；
- note↔syllable DP；
- melisma；
- confidence；
- manual relink。

## Phase 8 — Scansion Reports

- universal lyric grid；
- phrase sheet；
- Markdown/PDF；
- report templates。

## Phase 9 — Productization

- model manager；
- installer；
- updater；
- diagnostics；
- crash recovery；
- license attribution；
- CI release pipeline。

---

# 51. Milestone 验收标准

## M0

导入音频、播放、手工标 note/lyrics、保存重开、导出 JSON。

## M1

自动 pitch 可显示，低 confidence 可见，且不改变用户编辑。

## M2

自动 note 足以作为编辑起点，MIDI 导出正确。

## M3

BPM/beat grid 有多个候选，用户可快速修正。

## M4

英文/日文/中文歌词至少可进入统一 token/syllable/phoneme 模型。

## M5

可生成跨语言 lyric grid，并处理 melisma。

## M6

完整 `.vocalproj`、模型管理、自动更新与稳定 release。

---

# 52. ADR（Architecture Decision Record）建议

至少建立：

- ADR-001 Tauri vs Electron
- ADR-002 Rust core
- ADR-003 Python research-only
- ADR-004 Canonical time model
- ADR-005 Observation/Override separation
- ADR-006 Analysis DAG
- ADR-007 ONNX preferred runtime
- ADR-008 Project ZIP container
- ADR-009 Canvas/WebGL timeline
- ADR-010 Language provider interface
- ADR-011 Provider process protocol future
- ADR-012 Local-first privacy

每次重大改变写 ADR，而不是只改代码。

---

# 53. GPT Astra6 实现约束

以下要求应视作后续代码生成的强制约束。

## 53.1 禁止事项

GPT Astra6 不得：

1. 将 domain structs 放进 React/Tauri adapter；
2. 将 Whisper/Basic Pitch 类型直接用于 domain；
3. 用 Python server 作为默认生产 backend；
4. 将 BPM 仅实现成 global `f64`；
5. 用数组 index 作为实体 ID；
6. 自动分析覆盖 user override；
7. 删除 raw observation 只保留 resolved output；
8. 将 waveform/pitch 全量巨大数组经 JSON IPC 反复传输；
9. 用 DOM 节点逐点画 pitch curve；
10. 把所有 analysis 写进一个 monolithic function；
11. 在 domain crate 引入 UI、Tauri 或具体模型依赖；
12. 在没有 migration 的情况下修改持久化 schema；
13. 在实时 audio callback 中执行 blocking/inference/allocation-heavy 工作；
14. 把语言判断写成无限增长的 `if language == ...` 主逻辑；
15. 用未经校验的模型输出直接显示为“确定事实”。

## 53.2 必须事项

每次实现新 analyzer：

- 实现统一 Analyzer contract；
- 声明 dependencies；
- 声明 provider/version；
- 生成 provenance；
- 定义 cache key；
- 支持 cancellation；
- 提供 fixture test；
- 不覆盖 user override。

每次新增持久化字段：

- 更新 schema version（如 breaking）；
- 写 migration；
- 添加 round-trip test。

每次新增语言：

- 通过 `LanguageProvider`；
- 不改核心 alignment 算法的语言分支，除非以 generic features 表达不了；
- 添加语言级 fixtures。

---

# 54. GPT Astra6 推荐开发方式

对于每个阶段，先输出：

1. 设计变更；
2. crate/module 边界；
3. public interfaces；
4. invariants；
5. tests；
6. 然后再写代码。

禁止一次生成数千行未经架构审查的完整应用。

建议每个 PR/任务做到：

```text
small coherent vertical slice
```

例如：

> Import WAV → decode → waveform cache → viewport render

而不是：

> “实现整个音频系统”。

---

# 55. 第一批应创建的 Domain 类型

优先：

```text
EntityId
Seconds
Samples
Tick
TimeSpan
MusicalSpan
TempoMap
MeterMap
AudioSource
VocalProject
VocalTrack
PitchObservation
PitchTrack
NoteEvent
Phrase
LyricDocument
LyricToken
LinguisticFeatures
AlignmentEdge
Confidence
Provenance
Revision<T>
```

以及 tests。

不要先写 Whisper integration。

---

# 56. 第一批代码任务建议

### Task 1
建立 Cargo workspace + Tauri React skeleton。

### Task 2
实现 `vocal-domain`，100% 无 UI/model dependency。

### Task 3
实现 typed time + TempoMap sec/tick conversion + tests。

### Task 4
实现 project schema v1 + migration framework + roundtrip。

### Task 5
实现 WAV/Symphonia import 与 AudioSource metadata。

### Task 6
实现 waveform pyramid artifact。

### Task 7
实现 Timeline viewport 与 waveform rendering。

### Task 8
实现 playback cursor synchronization。

### Task 9
实现 manual NoteEvent CRUD + undo/redo。

### Task 10
实现 LyricToken + manual alignment + primitive scansion renderer。

完成这十步之后，再进入自动分析。

---

# 57. Future Extensions

架构应允许但暂不实现：

- full mix source separation；
- multi-vocal harmony tracks；
- singer identity independent models；
- VST/AU plugin；
- live recording；
- real-time note tracking；
- cloud batch analysis；
- collaborative annotations；
- lyric suggestion；
- rhyme assistance；
- tone-aware Mandarin lyric fitting；
- stress-aware English fitting；
- Vocaloid/SynthV export；
- training data annotation mode；
- API/CLI/headless analysis；
- mobile viewer；
- plugin marketplace。

如果 core/domain 正确，这些不需要推翻项目。

---

# 58. 风险与应对

## 风险 A：Singing ASR 不够准

应对：

- text input + forced alignment 是一等 workflow；
- ASR 只是 optional observation；
- 用户编辑快速；
- provider 可替换。

## 风险 B：solo vocal BPM 模糊

应对：

- hypotheses；
- half/double alternatives；
- manual downbeat；
- quantization score；
- 不伪装成绝对正确。

## 风险 C：跨语言膨胀

应对：

- common hierarchy + features；
- language provider；
- capability flags；
- GenericProvider fallback。

## 风险 D：ML runtime 打包困难

应对：

- Python research-only；
- ONNX preferred；
- mature native runtimes separately wrapped；
- models outside app bundle。

## 风险 E：UI 性能

应对：

- viewport query；
- tiled cache；
- Canvas/WebGL；
- Rust owns large data；
- no giant JSON。

## 风险 F：用户修正被重分析破坏

应对：

- Revision model；
- raw observation separate；
- override precedence；
- explicit reset。

---

# 59. 建议的“最小但正确”v0.1

v0.1 不需要 Whisper，不需要 AI。

只需：

1. import vocal；
2. waveform；
3. playback；
4. timeline；
5. manual note；
6. manual lyric；
7. manual alignment；
8. tempo/grid；
9. lyric scansion view；
10. save/load。

如果这版已经让真实用户愿意拿它替代“DAW 截图 + 手打词格”，说明 product foundation 正确。

随后自动分析只是不断减少人工劳动。

---

# 60. 最终架构判断

整个产品的核心不是：

```text
Whisper + Basic Pitch + UI
```

而应是：

```text
              ┌───────────────────────┐
              │ Canonical Vocal Model │
              └───────────▲───────────┘
                          │
                 Fusion / Revision
                          ▲
       ┌──────────────────┼──────────────────┐
       │                  │                  │
     Pitch              Lyrics             Rhythm
       │                  │                  │
   providers           providers           DSP/ML
       │                  │                  │
       └──────────────────┴──────────────────┘
                          ▲
                        Audio
```

模型一定会过时；领域模型、编辑模型、时间系统、分析 DAG、缓存、语言抽象、alignment 与用户工作流才是长期资产。

**任何实现决策都应优先保护这几个长期资产。**

---

# Appendix A — 当前推荐组件（2026-09 调研基线）

> 版本会变化。实现时应重新核对官方文档、license 与当前稳定版本，不应把本文版本号永久写死。

- **Tauri 2**：Rust + OS WebView 桌面应用框架，适合作为 shell/IPC 边界。
- **Symphonia**：Pure Rust audio decode/demux，适合作为主 decode backend。
- **CPAL**：Rust 跨平台低层 audio I/O。
- **Rubato**：Rust resampler，支持 fixed-ratio FFT 与高质量 sinc 等路径。
- **ONNX Runtime / `ort`**：Production model inference preferred runtime。
- **whisper.cpp**：本地 ASR provider 候选，支持多平台与多种硬件 backend。
- **Spotify Basic Pitch**：Audio-to-MIDI / pitch-bend baseline，支持多 runtime serialization，包括 ONNX。
- **torchcrepe/CREPE**：Pitch research baseline，带 periodicity/confidence 与 Viterbi decoding。
- **WhisperX**：研究 word-level/phoneme alignment pipeline。
- **Montreal Forced Aligner 3.x**：Forced alignment benchmark/toolchain。

---

# Appendix B — 官方资料入口

以下仅作为工程核验入口，所有许可证与版本应在真正加入依赖时再次检查：

- Tauri Architecture: https://v2.tauri.app/concept/architecture/
- Symphonia docs: https://docs.rs/symphonia/
- CPAL docs: https://docs.rs/cpal/
- Rubato docs: https://docs.rs/rubato/
- ONNX Runtime: https://onnxruntime.ai/
- Rust `ort`: https://docs.rs/ort/
- whisper.cpp: https://github.com/ggml-org/whisper.cpp
- Spotify Basic Pitch: https://github.com/spotify/basic-pitch
- torchcrepe: https://github.com/maxrmorrison/torchcrepe
- WhisperX: https://github.com/m-bain/whisperX
- Montreal Forced Aligner: https://montreal-forced-aligner.readthedocs.io/

---

# Appendix C — 给 GPT Astra6 的启动指令模板

将本设计文档提供给实现模型后，可附加：

```text
你是该项目的首席软件工程实现代理。

本 Software Design Document 是架构约束而不是灵感参考。
除非我明确批准 Architecture Decision Record (ADR) 变更，否则不得绕过其中的核心边界。

开始任何新阶段前：
1. 指出本阶段涉及的 crates/modules；
2. 给出 public interfaces；
3. 列出 invariants；
4. 列出会新增/失效的 cache 与 analysis dependencies；
5. 列出 tests；
6. 确认没有破坏 user overrides、provenance 与 schema migration；
7. 然后实现。

优先完成小而完整的 vertical slice，不生成不可审查的大型 monolithic patch。

所有自动分析都必须：
- provider-independent；
- 有 confidence；
- 有 provenance；
- 可缓存；
- 可取消；
- 不覆盖 user override；
- 可被更换模型重新计算。

遇到具体模型能力不足时，不允许为适配模型而污染 Canonical Domain Model；应新增 adapter/provider 或 raw observation。
```

---

**End of Software Design Document v0.1**
