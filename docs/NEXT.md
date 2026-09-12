# 下一阶段

目标：实现首个手工 note/lyrics vertical slice，让用户事实可编辑、保存、重开并导出基础 JSON。

推荐 vertical slice：先按 SDD 的 user override 边界补齐 domain note/lyric 实体与明确校验，再以 schema migration 持久化；应用层提供按稳定 identity 的新增/修改/删除命令，UI 在时间轴上编辑并标示用户来源。JSON 导出只序列化当前 `VocalProject` 用户事实与版本信息，不导出分析 PCM/波形缓存。

依赖：在改 schema 前写清 note pitch/time/duration、lyric text/syllable 关联和 override provenance 的最小不变量，并沿用现有 migration/原子保存流程。若 SDD 对首个实体字段不足以作唯一实现选择，先提交 ADR 供用户批准；不要让分析结果覆盖手工实体。

暂缓的验证缺口：真实 Windows 文件选择器、扬声器播放与窗口关闭仍需交互式原生验收，现有 MockRuntime、浏览器测试和文件系统 verifier 均不能替代。播放的设备选择/重采样/loop/延迟补偿、长音频分块分析与 undo/redo 保持独立后续切片。
