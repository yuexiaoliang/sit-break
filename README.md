# Sit Break

[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11-0078d4)](https://github.com/yuexiaoliang/sit-break)
[![Tauri](https://img.shields.io/badge/built%20with-Tauri%202-FFC131)](https://tauri.app)
[![Rust](https://img.shields.io/badge/language-Rust-DEA584)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Release](https://img.shields.io/github/v/release/yuexiaoliang/sit-break)](../../releases)
[![CI](https://github.com/yuexiaoliang/sit-break/actions/workflows/ci.yml/badge.svg)](../../actions/workflows/ci.yml)

**A tiny, beautiful sitting reminder for Windows.** Sit Break lives in your system tray, counts down quietly, and gently pops up a reminder when it is time to stand up and move — because forgetting to take breaks is how backs get ruined.

[English](README.md) | [简体中文](README.zh-CN.md)

<!-- Keywords: sedentary reminder, sitting reminder, break reminder, stand up reminder, desk break, floating countdown timer, system tray app, 久坐提醒, Windows 11, Tauri, Rust -->

## Why Sit Break

- **Tiny** — a 4 MB standalone executable; when idle it runs with **zero windows and zero webviews**, just a tray icon and a floating timer ball
- **Beautiful** — glassmorphism floating ball with a live seconds countdown, gradient progress ring, and a glowing dot that travels the ring
- **Smart** — system-wide idle detection: walk away for a coffee and the timer resets, so being away never counts as "sitting"
- **Bilingual** — Chinese & English UI, follows your system language or can be set manually

## Features

| | |
|---|---|
| ⏱ Floating countdown ball | Always-on-top, draggable, scroll-wheel resize (40–140 px), position remembered |
| 🔔 Break reminder popup | Glassmorphism card in the corner with a random stretch tip and a snooze option |
| 🧘 Break countdown | Circular progress ring, auto-resumes work timer when the break ends |
| ↻ One-click reset | Reset the countdown anytime from the tray panel or floating ball menu |
| 💤 Idle detection | No keyboard/mouse for 2 minutes → timer resets automatically |
| 🚀 Launch at startup | Optional, one toggle in settings |
| 🔔 Sound cue | Gentle two-tone chime when a reminder pops up (optional) |
| ✏️ Custom tips | Write your own reminder tips, one per line, shown at random |
| 🌐 中文 / English | Follows system language, or pick one in settings |

## Download

Grab the latest installer or portable exe from [Releases](../../releases):

- `Sit Break_x.y.z_x64-setup.exe` — NSIS installer
- `Sit Break_x.y.z_x64_en-US.msi` — MSI installer
- `sit-break.exe` — portable, no installation needed

> Requires Windows 10/11 with WebView2 (pre-installed on Windows 11).

## Build from Source

Prerequisites: [Node.js 18+](https://nodejs.org), [Rust](https://rustup.rs), and on Windows the [MSVC build tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) + WebView2.

```bash
npm install
npm run tauri dev    # run in development
npm run tauri build  # produce exe + installers in src-tauri/target/release
```

## How It Works

```
Work timer ──0──▶ Reminder popup ──Take a break──▶ Break countdown ──done──▶ Fresh timer
    ▲                                                                    │
    └────────────── Reset anytime (tray panel / floating ball) ◀─────────┘
```

- The timer, tray, idle detection and window management all live in **Rust** — no background webviews, no timer throttling, minimal memory
- The floating ball, reminder popup and settings window are pure UI, created on demand

## Tech Stack

[Tauri 2](https://tauri.app) · Rust · TypeScript · Vite — no UI framework, hand-rolled CSS.

## CI & Releases

- **CI**（`.github/workflows/ci.yml`）：push / PR 时自动执行前端类型检查与构建（`tsc + vite`）、`cargo check`、`cargo clippy`
- **Release**（`.github/workflows/release.yml`）：推送 `v*` 标签时自动构建，发布 NSIS 安装包、MSI 安装包和便携版 exe 到 GitHub Releases

发布新版本：更新 `src-tauri/tauri.conf.json` 和 `package.json` 中的 `version`，然后：

```bash
git tag v0.x.y
git push origin v0.x.y
```

## License

[MIT](LICENSE)
