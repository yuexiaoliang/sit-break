# 开发与发布

本文件面向开发者，使用简体中文维护。用户文档按 [AGENTS.md](../AGENTS.md) 维护中英文两版。

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

[CI](../.github/workflows/ci.yml) 在 macOS 与 Windows 运行前端测试和构建、Rust 检查与测试，并执行 macOS 原生浮窗定位检查。[Windows 原生验收](../.github/workflows/windows-native-check.yml)在真实 WebView2 中检查浮窗、视频与恢复流程；原生文件选择器和部分 macOS 手动交互仍有未覆盖项，详见 [验收范围](native-webview-check.md)与 [设计检查记录](../design-qa.md)。

[Windows 测试版](windows-preview.md)使用独立配置，不会替代正式发布。发布前同步更新中英文 README、安装与使用说明、测试版说明及 `docs/releases/` 中的双语版本说明，核对语言切换链接和目标提交的检查结果。GitHub Release 正文同时提供简体中文和英文；开发与验收材料继续使用简体中文。在已同步、工作区干净的 `main` 分支执行：

```bash
npm run release -- minor --dry-run
npm run release -- minor
```

也支持 `patch`、`major` 或指定版本号。脚本同步 npm、Tauri、Rust 的版本及锁文件，提交后推送 `main` 和版本标签；[Release 工作流](../.github/workflows/release.yml)构建 macOS 通用包和 Windows x64 安装包。发布后核对两平台构建、下载附件与版本说明，确认附件齐全。

