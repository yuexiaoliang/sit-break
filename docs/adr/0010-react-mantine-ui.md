# 使用 React 与 Mantine 统一应用界面

设置、托盘面板、休息浮窗和 Windows 悬浮计时器统一使用 React、TypeScript 与 Mantine，以减少原生表单控件在 WKWebView 和 WebView2 中的外观差异，并共用焦点、键盘操作、弹层和减少动态效果的处理。保留日程校验、`24:00` 时间适配、视频导入与播放生命周期、原生窗口定位等业务边界；计时状态和持久化继续由 Rust 负责，前端组件不复制计时规则。

采用雾紫强调色和浅白／柔黑背景。主题支持浅色、深色与跟随系统，保存到本机后各窗口同步；这扩展了 [ADR 0007](0007-guidance-follows-system-theme.md) 中仅跟随系统的规则，继续要求主题和读秒更新不得重建视频或改变播放状态。设置草稿切换侧栏页面后保留；“编辑日程”只导航到日程安排，是否执行仍由“使用日程”开关及保存操作决定。

代价是引入 React 与组件库运行时，并将前端兼容基线明确为 Safari／WKWebView 15.4+ 与 Chromium／WebView2 108+。构建目标不能代替 CSS 或系统 WebView 验证；Windows 使用持续更新的 WebView2，macOS 需更新系统 WebKit。调研与替代方案见 [选型记录](../research/ui-stack-2026-10-16.md)。
