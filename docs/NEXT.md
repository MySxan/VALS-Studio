# 下一阶段

目标：补齐真实原生桌面工程工作流验收，让自动检查覆盖实际窗口生命周期。

推荐 vertical slice：启动构建产物 → New → Import WAV → Save → 关闭并重启 → Open → 移动源并 Relink → Save As → 未保存修改时取消/确认关闭。验证真实窗口、IPC 和文件状态；文件选择器或关闭行为若只能人工验证，明确记录证据和未通过项，不能用 MockRuntime 结果替代。

依赖：Windows 原生自动化/驱动的可用性诊断、独立临时测试工程与进程清理、明确自动/人工验收边界。不改变 Canonical Domain Model，不把测试控制入口放入生产 IPC。

尚未完成：System 模式的完整干净环境验证、macOS/Linux 与 Unix symlink 测试、远程 CI 执行。关闭这些验证缺口后，再单独选择长音频分块分析或播放切片。
