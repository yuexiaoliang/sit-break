# Windows 测试版

PR 更新后，[Windows 测试版工作流](https://github.com/yuexiaoliang/sit-break/actions/workflows/windows-preview.yml)会构建 Windows 10/11 x64 安装包。打开对应提交的成功运行记录，在 Artifacts 中下载 `Sit-Break-Windows-x64-preview`，下载需要登录 GitHub。文件保留 14 天，不会创建正式 Release 或版本标签。

解压后运行 `Sit-Break-Preview-x64-setup.exe` 安装；也可运行 `Sit-Break-Preview-x64-portable.exe`。建议先退出正在运行的 Sit Break，避免多份提醒同时出现。安装程序会按需安装 WebView2，可能需要联网；免安装版本要求电脑已安装 WebView2 Runtime。测试包未使用 Windows 代码签名证书。

应用名为 **Sit Break Preview**，标识为 `com.sitbreak.preview`，设置和运行进度保存在 `%APPDATA%\com.sitbreak.preview`，与正式版分开。解压目录中的使用说明记录完整源码提交，`SHA256SUMS.txt` 用于核对安装包和可执行文件。

启动后点击右下角托盘图标或悬浮计时条，进入设置。关闭“使用日程”可体验简单计时，开启后可编辑日程、执行时段和单轮安排。快速体验提醒可先设置工作 1 分钟、休息 1 分钟。实机验收范围见[日程验证记录](schedule-verification.md)。
