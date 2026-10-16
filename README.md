<p align="center">
  <img src="docs/design/logo/mark.svg" alt="Sit Break 标志" width="72" height="72" />
</p>

# Sit Break · 久坐提醒

[![macOS](https://img.shields.io/badge/macOS-12%2B-000000)](https://github.com/yuexiaoliang/sit-break/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078d4)](https://github.com/yuexiaoliang/sit-break/releases/latest)
[![Release](https://img.shields.io/github/v/release/yuexiaoliang/sit-break)](https://github.com/yuexiaoliang/sit-break/releases/latest)
[![CI](https://github.com/yuexiaoliang/sit-break/actions/workflows/ci.yml/badge.svg)](https://github.com/yuexiaoliang/sit-break/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

**按自己的节奏工作，到点起来活动。** Sit Break 是一款在本机运行的久坐提醒工具，常驻 macOS 菜单栏或 Windows 系统托盘。选择固定的工作／休息节奏，或按日期、时段安排日程；休息时可以显示自定义文字和自己导入的本地视频。

[下载最新正式版](https://github.com/yuexiaoliang/sit-break/releases/latest) · [v0.5.0 更新说明](docs/releases/v0.5.0.md) · [日程使用说明](docs/schedules.md)

## v0.5.0 有什么变化

- **统一界面**：设置、托盘面板、休息浮窗和 Windows 悬浮计时条共用控件与雾紫配色，支持浅色、深色和跟随系统。
- **明确的计时方式**：顶部切换“简单计时／日程计时”，只展示当前方式的配置；切换保留草稿，保存后生效。
- **本地视频指引**：为提醒条目或指定轮次导入本地视频，可与文字搭配，也可单独使用。
- **更紧凑的休息浮窗**：单行文字自动收紧，长文独立滚动；视频、倒计时和结束按钮保持清楚可见。
- **更顺手的设置**：侧栏导航、日程概览、空状态引导、圆角窗口和轻微过渡；尊重系统的减少动态效果设置。

## 界面预览

下图使用示例数据展示新版界面：左侧为浅色简单计时，右侧为深色日程计时。

<table>
  <tr>
    <td align="center"><img src="docs/ui-design/app-refresh/timer-mode-simple-light.png" alt="浅色简单计时：工作 45 分钟、休息 5 分钟" width="360" /></td>
    <td align="center"><img src="docs/ui-design/app-refresh/timer-mode-scheduled-dark.png" alt="深色日程计时：已启用日程及执行时段概览" width="360" /></td>
  </tr>
</table>

<p align="center">
  <img src="docs/ui-design/app-refresh/reminder-video.png" alt="休息浮窗：本地视频、倒计时与文字说明" width="624" />
</p>

## 下载与安装

前往 [最新正式版](https://github.com/yuexiaoliang/sit-break/releases/latest)，按平台选择文件：

| 平台 | 文件 | 使用方式 |
| --- | --- | --- |
| macOS | `Sit.Break_<版本>_universal.dmg` | 支持 Apple Silicon 和 Intel；打开后将应用拖入 Applications |
| Windows x64 | `Sit.Break_<版本>_x64-setup.exe` | 推荐的安装程序，可按需安装 WebView2 |
| Windows x64 | `Sit.Break_<版本>_x64_en-US.msi` | MSI 安装程序 |
| Windows x64 | `sit-break.exe` | 免安装程序，需事先安装 WebView2 Runtime |

**兼容范围**：macOS 12+，系统 WKWebView 需支持 WebKit 15.4+，建议安装系统及浏览器组件更新；Windows 10/11 x64，使用 Chromium 108+ 的 WebView2，建议保持 Runtime 更新。设置、计时和已导入视频在本机使用，无需登录；Windows 安装时若缺少 WebView2，安装程序可能需要联网补齐。

macOS 使用本地 ad-hoc 签名，尚未经过 Apple 公证；Windows 安装包未使用代码签名证书。

升级前退出正在运行的旧版，覆盖安装即可保留原正式版设置。`Sit Break Preview`、`Sit Break UI Check` 等独立测试包与正式版使用不同配置目录，测试包中的日程不会自动迁移到正式版。

## 开始使用

安装后启动应用：

- **macOS**：点击顶部菜单栏的标志或倒计时，打开“设置”；应用不显示 Dock 图标或桌面悬浮计时条。
- **Windows**：点击系统托盘图标或桌面悬浮计时条，打开“设置”；托盘图标可能位于展开箭头中。

### 简单计时

进入“计时与休息”，选择“简单计时”，设置工作时长、休息时长并保存。首次使用默认为工作 45 分钟、休息 5 分钟，也可以用快捷按钮选择时长。工作为 1–240 分钟，休息为 1–60 分钟，均为整数。

工作结束先提醒，点击“开始休息”后才开始本次休息倒计时。准备稍后休息时，可以选择“再等等”；休息结束后进入下一轮。想快速体验，可先设为工作 1 分钟、休息 1 分钟。

### 日程计时

在同一页选择“日程计时”，通过“添加日程”或“编辑日程”进入详细设置，也可以从侧栏“日程安排”进入。

1. 填写日程名称，选择指定日期执行一次，或按勾选的星期每周重复。
2. 添加执行时段，填写开始、结束时间及默认工作／休息时长。结束时间支持 `24:00`；跨午夜请拆成两天安排。
3. 展开“单轮调整（可选）”，只填写需要不同的轮次、时长、休息内容或本地视频。时长留空表示继承本时段默认值。
4. 点击“预览日程”查看预计安排，再保存设置。预览不会保存草稿，也不会启动计时。

| 计时方式 | 执行规则 | 没有有效执行时段时 |
| --- | --- | --- |
| 简单计时 | 按一组全局工作／休息时长循环 | 继续简单计时 |
| 日程计时 | 按启用日程的时段、默认时长和单轮调整执行 | 保持待机，不自动退回简单计时 |

日程概览只展示已启用的日程及其安排。两种计时方式互相保留配置；“编辑日程”和侧栏入口只负责导航，不切换计时方式。提示音、离开自动重置、提醒内容等设置共用。

例如，工作日可设置 `09:00–12:00` 与 `13:30–18:00` 两个执行时段，中间留作午休。时段之间不提醒；时段结束时停止工作和尚未开始的休息提醒，已经开始的休息可以完成。更完整的轮次、冲突、恢复与睡眠规则见 [日程使用说明](docs/schedules.md)。

### 文字与本地视频

在“提醒内容”中逐条设置提醒小字和动作指引，或在日程的单轮调整中为指定轮次设置内容。

- 文字与视频至少保留一个，可以只有文字、只有视频，或同时配置。
- 动作指引通过系统文件选择器导入本地 MP4／M4V，推荐使用 H.264 编码的 MP4。应用检查能否播放；不支持时可转为兼容编码后重新导入。
- 应用保存视频副本，原文件移动或删除不影响已导入指引；修改源视频后需重新导入。取消或导入失败时保留原配置。
- 仅支持本地视频文件，不支持网址、在线内容、HTML 文件或在输入框粘贴 HTML 源码。

某轮配置了文字或视频时，使用该轮完整的内容组合；两者都未配置时，随机选择一条全局提醒。休息内容按原文展示，可以描述多项活动，但整次休息共用一个倒计时。

点击“开始休息”后，视频默认静音循环，可播放／暂停、拖动进度或开启声音；这些操作不改变休息倒计时。切换主题和倒计时更新保持播放器状态，提前结束休息会停止播放。

## 外观与日常操作

在侧栏底部或“通用设置”中选择浅色、深色或跟随系统，保存后各窗口同步。切换外观不会清空编辑中的内容，也不会重启正在播放的视频。系统启用减少动态效果时，应用减少非必要过渡。

- **暂停与重置**：暂停保持到手动恢复；重置重新计当前轮工作，提前结束休息进入下一轮。
- **离开自动重置**：按本轮休息时长判断是否离开够久，可关闭；工作中离开够久重置当前轮，提醒后离开够久视为本轮已休息。
- **退出恢复**：正常退出后，在同一天、同一有效日程时段重开可恢复本轮进度和暂停状态；应用关闭期间不累计计时。
- **本机设置**：支持登录电脑后自动启动、提示音开关，以及中文／English 界面；Windows 可开关悬浮计时条并调整大小。
- **草稿与快捷键**：切换侧栏和计时方式保留草稿；`⌘S`／`Ctrl+S` 保存。`Esc` 优先关闭当前下拉或弹层。

## 从源码运行

需要 Node.js 22.12+ 与 Rust stable。macOS 需 Xcode Command Line Tools；Windows 需 MSVC 构建工具与 WebView2。

```bash
npm ci
npm run tauri dev
```

构建当前平台的应用和安装包：

```bash
npm run tauri build
```

产物位于 `src-tauri/target/release/bundle/`；Windows 免安装程序位于 `src-tauri/target/release/sit-break.exe`。macOS 通用版本需先安装两个 Rust 目标：

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

项目使用 **Tauri 2、Rust、React、TypeScript、Mantine 和 Vite**。Rust 负责计时、日程、运行进度、窗口与本地资源；前端负责界面与交互。关闭设置或托盘面板不会中断计时。

## 开发验证与发布

```bash
npm test
npm run build
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
```

[CI](.github/workflows/ci.yml) 在 macOS 与 Windows 运行前端测试和构建、Rust 检查与测试，并执行 macOS 原生浮窗定位检查。[Windows 原生验收](.github/workflows/windows-native-check.yml)在真实 WebView2 中检查浮窗、视频与恢复流程；原生文件选择器和部分 macOS 手动交互仍有未覆盖项，详见 [验收范围](docs/native-webview-check.md)与 [设计检查记录](design-qa.md)。

[Windows 测试版](docs/windows-preview.md)使用独立配置，不会替代正式发布。发布前更新本 README、相关使用说明及 `docs/releases/` 中的版本说明，确认目标提交的检查结果。在已同步、工作区干净的 `main` 分支执行：

```bash
npm run release -- minor --dry-run
npm run release -- minor
```

也支持 `patch`、`major` 或指定版本号。脚本同步 npm、Tauri、Rust 的版本及锁文件，提交后推送 `main` 和版本标签；[Release 工作流](.github/workflows/release.yml)构建 macOS 通用包和 Windows x64 安装包。发布后核对两平台构建、下载附件与版本说明，确认附件齐全。

## 许可证

[MIT](LICENSE)
