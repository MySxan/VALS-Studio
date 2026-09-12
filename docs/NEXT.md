# 下一阶段

目标：实现首个独立播放 vertical slice，让已核验 WAV 可从当前视口位置播放、暂停和 seek，并让播放光标与后端时钟同步。

推荐 vertical slice：新增独立 `PlaybackEngine` adapter 与 application playback session；只播放当前已核验 source，不复用 analysis PCM，不把播放状态写入 `VocalProject`。IPC 提供 load/play/pause/seek/status/stop 的粗粒度命令，前端 transport 轮询轻量状态并绘制独立光标。工程切换、Relink、源失效或应用关闭必须停止旧播放；音频 callback 不做锁等待、日志、分析或大块分配。

依赖：先确定 CPAL 设备输出与可测试时钟/无设备 fallback 的 adapter 边界，复用现有路径解析和 source 内容核验；新增依赖前更新许可清单。第一切片只承诺 WAV PCM，播放不是 analyzer，不新增 analysis artifact/cache key 或持久化 schema。

暂缓的验证缺口：真实 Windows 文件选择器与窗口关闭仍需交互式原生验收，现有 MockRuntime、浏览器测试和文件系统 verifier 均不能替代。播放完成后继续手工 note/lyrics、JSON 导出与 undo/redo；长音频分块分析保持独立后续切片。
