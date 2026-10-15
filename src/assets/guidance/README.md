# 内置下蹲示范视频

`squat-demo.mp4` 是本项目原创绘制的侧面下蹲动画：H.264 Baseline、640×360、30 fps、6 秒、无音轨。人物、动作和背景由项目生成器绘制，无第三方视频、图片或音乐素材。视频中的文字使用 macOS 系统字体渲染；成品随本仓库 MIT 许可分发，可随应用打包。

可复现生成器为 `scripts/make-squat.m`，使用 macOS 自带的 AVFoundation、CoreGraphics 和 CoreText，不需要转码或下载外部素材。在仓库根目录执行：

```sh
clang -fobjc-arc -fno-modules -framework AVFoundation -framework CoreGraphics -framework CoreMedia -framework CoreVideo -framework CoreText -framework ImageIO -framework Foundation scripts/make-squat.m -o /tmp/sit-break-make-squat
/tmp/sit-break-make-squat src/assets/guidance/squat-demo.mp4
```

生成器同时输出中间帧 `squat-demo.mp4.png` 供人工检查；该截图不属于播放器资源，可检查后删除。动画是轻量动作展示，用户按自己的身体状况和节奏活动。

前端直接引用本资源，由 Vite 复制到构建产物并交给 Tauri 打包；无需联网，也不依赖开发机绝对路径。

## HTML 示范与作者模板

`stretch-demo.html` 是本项目原创的单文件示范，展示短要点、可折叠说明和“已读”按钮；CSS、JavaScript 全部内嵌，没有在线资源。`guidance-template.html` 使用相同的响应式布局和完整语义配色，供用户替换内容。两者随本仓库 MIT 许可分发。

内置 HTML 由 Rust `include_str!` 固定打包，再经与本地导入 HTML 相同的受控读取和隔离 iframe 展示，不作为拥有原生能力的应用页面执行。产品内“保存示范 HTML…”与“保存 HTML 模板…”使用原生保存对话框，导出相同原文；取得的示范可以从普通本地 HTML 入口重新导入。

`prompt-intro.txt` 与完整模板共同组成产品中可复制的生成提示词。模板是产品配色表的色值来源，调整配色时同时保持 `src/guidance-theme.css` 和 `docs/guidance-theme.md` 的公开约定一致。作者使用路径与单文件约定见 `docs/html-authoring.md`。
