# 内置下蹲示范视频

`squat-demo.mp4` 是本项目原创绘制的侧面下蹲动画：H.264 Baseline、640×360、30 fps、6 秒、无音轨。人物、动作和背景由项目生成器绘制，无第三方视频、图片或音乐素材。视频中的文字使用 macOS 系统字体渲染；成品随本仓库 MIT 许可分发，可随应用打包。

可复现生成器为 `scripts/make-squat.m`，使用 macOS 自带的 AVFoundation、CoreGraphics 和 CoreText，不需要转码或下载外部素材。在仓库根目录执行：

```sh
clang -fobjc-arc -fno-modules -framework AVFoundation -framework CoreGraphics -framework CoreMedia -framework CoreVideo -framework CoreText -framework ImageIO -framework Foundation scripts/make-squat.m -o /tmp/sit-break-make-squat
/tmp/sit-break-make-squat src/assets/guidance/squat-demo.mp4
```

生成器同时输出中间帧 `squat-demo.mp4.png` 供人工检查；该截图不属于播放器资源，可检查后删除。动画是轻量动作展示，用户按自己的身体状况和节奏活动。

前端直接引用本资源，由 Vite 复制到构建产物并交给 Tauri 打包；无需联网，也不依赖开发机绝对路径。
