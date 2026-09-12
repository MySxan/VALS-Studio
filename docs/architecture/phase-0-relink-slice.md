# Same-content Relink 切片

相对路径与 Save As 策略已由后续 [Project-relative 切片](phase-0-relative-source-slice.md) 扩展；以下路径策略是本切片最初实现。

闭环：打开离线工程 → 选择候选 WAV → 后台核验 hash / size / metadata → 恢复 source 路径及共享轨道波形 → 标记 dirty → 原子保存 → 新 application 实例重开并重新分析。

## 接口与边界

- `vocal-app::AppService::relink_track(project_id, generation, track_id, path) -> Result<String, AppError>` 返回可取消 job ID；Tauri `relink_track` 与 frontend `Backend.relinkTrack` 对应。
- `VocalProject::set_source_uri(id, uri)` 只修改一个现有 source 的位置，先校验再发布；不改变 project/source/track 身份、内容事实或其他工程字段。文件内容核验由 application 调用 audio adapter 完成，domain 不做 I/O。
- 复用 `WavImporter`、Decode → Waveform runtime、job polling、generation 和 viewport。没有新依赖、provider 或 schema。

## Invariants

候选必须与原 source 的 SHA-256、字节数和 metadata 完全一致。不同内容报告 `relink.candidate_changed`；原 source 的状态和关联保持不变。导入、核验、分析全部成功，且提交时未取消、job / project generation 仍匹配，才发布 URI 和派生波形。取消、读取失败、错误候选或分析失败均不产生工程修改。

成功恢复同一 source 的所有引用轨道；不会新增 source/track。路径实际改变才新增 dirty，已有 dirty 不会清除。已有分析证据在候选处理期间保留；失败不覆盖证据。分析仍是派生状态，不写入 `.vocalproj`，不写 Raw Observation、Resolved Value 或 User Override。

## 路径与保存策略

本切片仅支持用户显式选择的绝对位置。成功 Relink 使用 importer 的 canonical absolute fallback，并将旧 relative path 清空，避免保留指向旧位置的相对关联。自动相对路径解析尚未实现。

Save As 只保存工程副本，保留当前 source URI，不移动或复制 WAV，不实现 copy-on-save-as。`ProjectStore::load` 保持 metadata-only；需要显示波形时由 application 核验源。仍使用 schema v2、现有 migration 和原子写入。没有 ADR 边界变更。

## Cache / analysis dependencies

缓存键仍依赖内容与 metadata、provider/model/settings 和上游 artifact hashes，不依赖 URI。Relink 仍先核验实际文件，再运行既有 cache-aware pipeline；命中时保留完整 provenance/confidence。缓存被淘汰或 application 重启时正常重新计算。失败或取消可留下合法派生 cache，但不能发布语义修改。

## 验证

核心覆盖共享 source 恢复、身份/内容/轨道不变、同路径不新增 dirty、缓存 provenance 保留、保存及新实例重开、错误 hash 候选、缺失候选、stale generation、提交前取消，以及 domain URI 更新失败的原子性。前端覆盖恢复/保存、候选失败/取消、对话框取消、IPC 错误、重复操作防护及 dispose 后取消迟到任务。Tauri MockRuntime 通过真实 store / runtime 验证 Relink 参数、结果和 waveform。

本机通过 66 项核心测试、19 项前端测试、1 项扩展 IPC 闭环测试，以及 fmt、Clippy、TypeScript/Vite、MSVC 原生构建。真实原生文件对话框及窗口关闭仍未端到端验收。
