# Windows 原生浮窗验收

独立工作流 `Windows 原生浮窗验收` 在拉取请求或手动触发时构建并启动真实 Windows exe，通过该进程的 WebView2 调试协议操作真实页面和公开应用命令。它使用 Node 22 自带的 WebSocket，不启动 Chrome，不替换 IPC，也不使用计时模拟页面。

工作流生成专用 `com.sitbreak.nativecheck` 构建，保存设置前再次通过应用接口检查标识。测试要求该标识的数据目录尚不存在，避免覆盖已有数据。执行失败时不会删除设置来重试。手动重复运行应使用新的 Windows 测试用户或独立 runner。

## 已实现的检查

- 用真实 `save_settings` 保存工作和休息各一分钟的全天日程，关闭应用以产生真实进度文件。
- 仅将这份真实快照的工作剩余时间缩为一秒，再启动 exe，验证实际恢复、提醒文字、开始休息和提前结束按钮。缩短等待不改变日程定义，不替代计时应用的业务测试。
- 采样实际休息倒计时，确认它推进；切换 Windows 当前用户的亮暗应用主题并广播系统通知，检查真实 WebView 的主题、颜色、计时和操作区域。
- 用 Win32 API 获取真实提醒窗口、客户区、DPI 和显示器工作区域，确认浮窗紧凑，倒计时和按钮位于视口内。
- 休息中正常退出并重开，确认当前轮、文字和剩余时间保留，再点击真实提前结束按钮进入下一轮。
- 保存仅含内置示范视频的简单计时提醒，实际等待一分钟进入提醒后点击开始休息，验证视频真实解码、静音自动播放、播放时间前进和循环；暂停和拖动不改变休息计时，tick 和系统主题切换保留同一个播放器与暂停位置，结束休息后清空媒体来源并移除元素。
- 复用同一 CDP 客户端调用独立在线视频模块：通过真实设置表单保存本机受控 HTTP 视频地址，验证无效地址保留配置、503 与重试、实际解码、暂停／拖动及设置刷新保留播放器、网络挂起后的超时提示与恢复重试。所有 HTTP 内容由测试服务器控制，结束时关闭连接；实际触发提醒仍经过真实进度文件和应用恢复入口。
- 本地视频使用明确的 `preimported-copy` fixture：在隔离应用目录准备符合真实存储格式的已导入 MP4 副本，再经公开设置选择不透明资源描述，验证真实受控协议播放、HEAD/Range、seek、当前轮替换保留、退出恢复旧引用和下一轮采用新引用。此路径不操作 Windows 文件选择器，不调用导入命令，不能当作文件选择／导入复制端到端证据。
- HTML 模块使用同样标记的 `.html` 副本，经过公开设置、真实恢复和开始按钮加载，检查真实 iframe 中的脚本、CSS 动画、内嵌与在线图片、滚动、系统主题与计时独立；退出重开恢复指引，结束休息销毁 frame。另用能力样例主动尝试原生命令、设置读写、真实存在文件和应用资源读取、嵌套 frame、顶层导航，并在宿主核对设置与计时状态。
- 内置 HTML 模块通过真实表单选定示范，并打开作者材料弹窗，检查完整提示词／模板、亮暗配色表和手动全选文本。休息中验证随包页面的交互、无外部资源、计时独立、主题保留和销毁。作者材料的原生保存对话框、实际导出与系统剪贴板写入不自动操作。

截图、各次应用输出和 `report.json` 保存到工作流 artifact。报告的 `status` 才表示这次运行结果；脚本或构建存在不表示验收通过。内置、在线、本地视频、本地 HTML 与内置 HTML 各有单独执行状态，只有完整执行后才记为通过；本地结果保留 fixture 标记和未覆盖项。新增内置 HTML／作者材料模块尚待对应的实际 Windows 运行；原生文件选择／导入、作者材料导出／剪贴板和外部浏览器实际打开也另行验收。

首轮基础路径在 [Windows 原生验收运行 38691851022](https://github.com/yuexiaoliang/sit-break/actions/runs/38691851022) 实际通过，源提交为 `f494262`：三次真实 exe 启动、公开命令与设置／进度保存、380×368 的可见提醒 HWND、系统亮暗、60→58 秒倒计时、51 秒退出恢复以及提前结束均有 JSON 和截图证据。该次运行明确未执行视频；不能据此声称新增的视频检查已通过。

随后 [运行 38693339989](https://github.com/yuexiaoliang/sit-break/actions/runs/38693339989) 在真实 WebView2 成功解码并静音播放 640×360 的六秒内置视频，播放、暂停及计时独立检查通过，但拖动进度失败。锁定 Tauri 的打包资源协议缺少字节 Range 响应，导致 WebView2 的内置视频 seek 回到开头。现对小型内置视频先读取为 Blob，结束或切换时撤销 object URL；在线视频仍直接按地址加载，原生 seek 断言保持不变。修复后的实际回归见下一段。

[运行 38694976591](https://github.com/yuexiaoliang/sit-break/actions/runs/38694976591)（`7686e0f`）随后实际通过保留的原生 seek 断言，内置视频在 light/dark 下保留同一元素及 1.2 秒暂停位置，循环和释放通过；在线视频 503／挂起超时／重试恢复亦通过。该轮没有本地 fixture 或 HTML 检查；新增模块仍须等待对应源提交的实际运行结果。

[运行 38697578332](https://github.com/yuexiaoliang/sit-break/actions/runs/38697578332)（`586e3ae`）实际通过九项检查，包括上述内置与在线视频，以及新增本地副本路径：真实 `guidance.localhost` 播放 640×360 视频、HEAD 200／244280 字节、Range 206／32 字节、seek 至 1.5 秒、保存替换时保留当前播放器、退出恢复旧副本、下一轮采用新副本和结束释放。该轮仍未操作 Windows 文件选择器，也未执行 HTML 场景。

[运行 38700003335](https://github.com/yuexiaoliang/sit-break/actions/runs/38700003335)（`88972ee`）实际通过新增三项单文件 HTML 检查。按钮计数 1、滚动 80px、动画 187→1395ms、内嵌与在线图真实加载、倒计时 180→179；系统亮暗保持同一 frame，原生窗口 420×460。能力页对设置读取、合法设置修改和结束休息的调用均未执行成功，真实文件／应用资源／本地媒体与 IPC 的读取均被阻止，嵌套 HTTP 请求为零；宿主设置完全不变，计时仅从 180→178，当前轮与指引不变。正常退出恢复与结束销毁也通过。该轮未包含内置 HTML／作者材料的新检查。

## HTML 能力检查的证据范围

`native-frame-context.mjs` 通过标准 CDP 的 frame tree 和默认 execution context 定位真实 iframe，不向页面提供额外能力。能力样例必须先由 `renderCapabilityFixture` 填入当前平台实际存在的文件 URL、宿主已成功读取的应用资源 URL、受控 HTTP 地址和合法完整设置。设置载荷保持本场景的完整配置，仅反转 `sound`，先由宿主通过公开入口实际接受并恢复基线，再用于真实尝试 `save_settings`；样例还尝试 `get_settings` 与 `end_break`，各等待至多 800ms，且不输出返回的设置或文件内容。

运行后必须从宿主公开接口核对设置完全不变、当前轮与休息状态保持、剩余时间只按墙钟下降，并确认宿主未导航。拒绝、未决或全局对象不存在均不能单独证明权限隔离。文件由测试先实际创建，应用资源由宿主先成功读取，避免把不存在的路径误判成安全结果。合法外链实际打开留给 CUA 验收，能力样例不自动打开外部 HTTP 浏览器。

## 本机运行

在线网页模块 `native-online-html-check.mjs` 提供本机 HTTP 样例和 `checkOnlineHtml(context)`，由主验收脚本复用同一原生会话。样例覆盖外部 CSS、按钮、动画、内部滚动、HTTP 重定向、挂起、断连、纯文本 503，以及保留原始 `X-Frame-Options: DENY` 和 `frame-ancestors 'none'` 的拒绝页；拒绝页不能通过下载重写而变为可展示。在线页面另执行实际能力攻击，包含对确实存在的导入视频副本的读取尝试。

在线 iframe 与本地 HTML 使用同一 opaque sandbox。宿主只授权当前休息选中的一个初始 HTTP(S) 地址；Windows 仅允许同一个导航 ID 的服务器重定向，完成后封闭此授权。就绪消息还须验证文档地址与当前会话；`load` 事件不作为成功证明。15 秒无法确认时显示“页面可能无法在此显示”，保留重试和浏览器入口，计时继续。网页自身可运行脚本的错误页未必能由跨域宿主识别，因此在线页面在就绪后也保留重试；外部网站保留自己的响应头和配色。

受控视频协议只向实际应用 origin 授予 CORS 读取许可，opaque 或其他远程页面不授予此许可；普通无 CORS 视频播放继续可用。调试构建可额外使用配置的开发 origin，发布构建不授予这项开发许可。

需要 Node 22、WebView2 Runtime、Windows 图形会话和干净的测试用户。先按工作流创建独立标识的配置，执行：

```powershell
npm run tauri build -- --debug --no-bundle --config "$env:TEMP/sit-break-native-check.json" -- --locked
node scripts/native-webview-check.mjs src-tauri/target/debug/sit-break.exe .scratch/native-webview-check
```

脚本临时修改当前测试用户的 `AppsUseLightTheme`，在 `finally` 恢复原值；强制杀死整个 runner 会阻止清理，因此应在专用测试环境运行。真实应用命令和设置仍由产品正常实现处理。正常退出失败时脚本会终止自己启动的进程，不接管用户的其他应用。

## 调试端口的边界

脚本向 exe 传递 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，使用临时本地 CDP 端口。Windows 提升权限进程可能忽略此环境设置，因此 debug 构建同时允许 `SIT_BREAK_NATIVE_CHECK_PORT` 指定同一端口。该辅助逻辑被 `cfg(all(windows, debug_assertions))` 限制，release 构建不包含此入口，不为验收关闭浏览器安全策略。

参考：[Microsoft WebView2 调试说明](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code)、[WebView2 browser flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags)、[Windows 图形会话 CI](https://learn.microsoft.com/en-us/windows/apps/develop/ai-assisted/testing#run-tests-in-ci)。

这套检查不覆盖物理睡眠、所有显示缩放组合、任意视频编码或任意第三方网页。macOS 使用独立的真实 WKWebView 验收，不把 Windows 结果推广到其他平台。
