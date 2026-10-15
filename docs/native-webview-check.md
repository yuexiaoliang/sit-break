# Windows 原生浮窗验收

独立工作流 `Windows 原生浮窗验收` 在拉取请求或手动触发时构建并启动真实 Windows exe，通过该进程的 WebView2 调试协议操作真实页面和公开应用命令。它使用 Node 22 自带的 WebSocket，不启动 Chrome，不替换 IPC，也不使用计时模拟页面。

工作流生成专用 `com.sitbreak.nativecheck` 构建，保存设置前再次通过应用接口检查标识。测试要求该标识的数据目录尚不存在，避免覆盖已有数据。执行失败时不会删除设置来重试。手动重复运行应使用新的 Windows 测试用户或独立 runner。

## 已实现的检查

- 用真实 `save_settings` 保存工作和休息各一分钟的全天日程，关闭应用以产生真实进度文件。
- 仅将这份真实快照的工作剩余时间缩为一秒，再启动 exe，验证实际恢复、提醒文字、开始休息和提前结束按钮。缩短等待不改变日程定义，不替代计时应用的业务测试。
- 采样实际休息倒计时，确认它推进；切换 Windows 当前用户的亮暗应用主题并广播系统通知，检查真实 WebView 的主题、颜色、计时和操作区域。
- 用 Win32 API 获取真实提醒窗口、客户区、DPI 和显示器工作区域，确认浮窗紧凑，倒计时和按钮位于视口内。
- 休息中正常退出并重开，确认当前轮、文字和剩余时间保留，再点击真实提前结束按钮进入下一轮。

截图、各次应用输出和 `report.json` 保存到工作流 artifact。报告的 `status` 才表示这次运行结果；脚本或构建存在不表示验收通过。当前基础脚本不选择视频或 HTML，报告明确标注媒体场景未运行；后续媒体任务可在同一个真实设置和休息路径增补检查。

## 本机运行

需要 Node 22、WebView2 Runtime、Windows 图形会话和干净的测试用户。先按工作流创建独立标识的配置，执行：

```powershell
npm run tauri build -- --debug --no-bundle --config "$env:TEMP/sit-break-native-check.json" -- --locked
node scripts/native-webview-check.mjs src-tauri/target/debug/sit-break.exe .scratch/native-webview-check
```

脚本临时修改当前测试用户的 `AppsUseLightTheme`，在 `finally` 恢复原值；强制杀死整个 runner 会阻止清理，因此应在专用测试环境运行。真实应用命令和设置仍由产品正常实现处理。正常退出失败时脚本会终止自己启动的进程，不接管用户的其他应用。

## 调试端口的边界

脚本向 exe 传递 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，使用临时本地 CDP 端口。Windows 提升权限进程可能忽略此环境设置，因此 debug 构建同时允许 `SIT_BREAK_NATIVE_CHECK_PORT` 指定同一端口。该辅助逻辑被 `cfg(all(windows, debug_assertions))` 限制，release 构建不包含此入口，不为验收关闭浏览器安全策略。

参考：[Microsoft WebView2 调试说明](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code)、[WebView2 browser flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags)、[Windows 图形会话 CI](https://learn.microsoft.com/en-us/windows/apps/develop/ai-assisted/testing#run-tests-in-ci)。

这套检查不覆盖物理睡眠、所有显示缩放组合、原生视频格式或任意第三方网页。macOS 使用独立的真实 WKWebView 验收，不把 Windows 结果推广到其他平台。
