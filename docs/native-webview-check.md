# 本地视频动作指引的原生验收

当前范围按 [ADR 0009](adr/0009-local-video-only-guidance.md) 收紧为从系统文件选择器导入本地视频。HTML 导入、展示和制作材料均已移除；此前包含 HTML 的报告与截图不作为当前范围的验收结果。

## 自动化检查

Windows 独立工作流构建并运行真实 exe，经 WebView2 调试接口检查产品实际页面、公开命令及持久化文件。专用标识为 `com.sitbreak.nativecheck`，必须使用全新的数据目录，不能删除已有用户设置来重试。工作流上传截图、应用日志和 `report.json`。

- 从真实保存的日程进度缩短工作等待，再经产品恢复入口触发提醒，核对文字、倒计时、系统主题、紧凑窗口、退出恢复与提前结束。
- 简单计时实际等待一分钟，播放已导入视频副本，检查静音循环、播放／暂停／进度、主题及 tick 保留、计时独立与结束释放。
- 视频休息使用横向布局；通过真实长文与键盘 PageDown 检查说明独立滚动，倒计时、视频和结束按钮位置保持。
- 对照 Win32 原生窗口矩形与显示器工作区，确认浮窗四边均在屏幕内；窗口内部控件可见不再作为屏幕定位正确的替代证据。
- 纯文字休息使用紧凑顶栏、线性进度和 17px 正文，移除大圆环；通过长文、键盘滚动和主题变化检查操作区与滚动位置保持。
- 单行纯文字经过真实恢复、提醒与开始休息入口后，按正文实际高度收紧为 244px；检查正文没有额外拉伸留白，设置刷新窗口后仍保持紧凑，结束按钮可用。所有提醒与休息状态均检查没有会越界的大范围 CSS 外投影。
- 视频副本验证受控协议、HEAD／Range、当前轮替换保留、退出恢复旧引用和下一轮新引用。
- 设置来源选项只包含“不使用指引”和“本地视频”，没有网页或制作材料入口。

媒体原生自动化使用明确标记的 `preimported-copy` fixture，预置格式与真实导入副本一致，但没有操作原生文件选择器，不能作为选择文件和导入命令的端到端证据。实际文件复制、删除源文件、替换／移除和保留引用另由 Rust 资源入口测试覆盖。

报告中各模块的 `status` 才表示对应执行结果。构建、脚本存在和其他版本的通过结果均不能代替本次原生结果；本地视频实现见 [PR #26](https://github.com/yuexiaoliang/sit-break/pull/26)，React／Mantine 界面与计时方式选择见 [PR #28](https://github.com/yuexiaoliang/sit-break/pull/28)。界面源提交 `6c1597a` 的[真实 WebView2 检查](https://github.com/yuexiaoliang/sit-break/actions/runs/38883983782)已通过，检查数据及截图可在对应运行的附件中查看。

## macOS 与手动交互的范围

macOS 已增加真实原生窗口的独立边界检查，直接调用产品共用的尺寸与定位入口，覆盖主线程首次视频休息、文字／视频切换、重复休息、后台线程切换，以及单行收紧为 244px、长文恢复到 368px，共八种路径。主线程路径用于复现同步 IPC 点击开始休息时的情况；仅检查后台线程无法发现本次越界。检查使用独立标识 `com.sitbreak.windowcheck`、空白 WebView，不读取用户设置，失败返回非零退出码；CI 在 macOS 上运行该检查。

2026-10-16 本机 2× 显示器上，旧实现的目标视频窗口宽 1248px，却按旧宽度 760px 定位，右边缘为 3320px，超出 2880px 工作区。修复后大小与位置使用同一目标物理尺寸，右边缘为 2832px，保留 48px 边距；上述六种路径均在工作区内。

此检查只确认原生尺寸与屏幕定位。macOS 本机的原生界面控制连接此前持续超时，WKWebView 播放、导入、主题和交互仍未记为通过；独立测试包构建和磁盘映像校验也不能代替这些交互证据。

尚未覆盖的手动验收项：

- [ ] macOS：本地视频的实际播放／交互、系统亮暗、退出重开和紧凑浮窗。
- [ ] macOS 与 Windows：从系统文件选择器导入 MP4；取消或失败保留旧配置，源文件移走后继续展示，替换和重开保持正确。
- [ ] macOS 与 Windows：纵向视频、长文字和较小屏幕下内容可读、内部滚动，倒计时和结束按钮始终可见。

本轮浏览器外观检查覆盖 540×960 竖屏视频、420×460 竖排回退、视频失败和纯文字状态。检查使用实际页面与隔离的计时演示数据，不能代替原生文件导入或 macOS 播放结果。设计对照与复核记录见仓库根目录 `design-qa.md`，截图位于 `docs/ui-design/break-window/`。

## 本机运行

macOS 原生定位检查需要图形会话，在前端构建后运行：

```sh
cargo run --locked --manifest-path src-tauri/Cargo.toml --example reminder_window_check -- /tmp/sit-break-window-check.json
```

Windows 需要 Node 22、WebView2 Runtime、图形会话和干净的测试用户。创建独立标识的 Tauri 配置后运行：

```powershell
npm run tauri build -- --debug --no-bundle --config "$env:TEMP/sit-break-native-check.json" -- --locked
node scripts/native-webview-check.mjs src-tauri/target/debug/sit-break.exe .scratch/native-webview-check
```

脚本会临时切换测试用户的应用亮暗主题，在结束时恢复原值。调试端口仅存在于 Windows debug 构建，发布构建不包含此入口。测试不关闭浏览器安全策略，不覆盖所有视频编码、显示缩放或物理睡眠场景。
