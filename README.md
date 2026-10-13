<p align="center">
  <img src="docs/design/logo/mark.svg" alt="Sit Break logo" width="72" height="72" />
</p>

# Sit Break

[![macOS](https://img.shields.io/badge/platform-macOS%2012%2B-000000)](https://github.com/yuexiaoliang/sit-break)
[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11-0078d4)](https://github.com/yuexiaoliang/sit-break)
[![Tauri](https://img.shields.io/badge/built%20with-Tauri%202-FFC131)](https://tauri.app)
[![Rust](https://img.shields.io/badge/language-Rust-DEA584)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Release](https://img.shields.io/github/v/release/yuexiaoliang/sit-break)](../../releases)
[![CI](https://github.com/yuexiaoliang/sit-break/actions/workflows/ci.yml/badge.svg)](../../actions/workflows/ci.yml)

**A small sitting reminder for macOS and Windows.** Sit Break lives in your macOS menu bar or Windows system tray, counts down quietly, and reminds you to stand up and move. Development and local validation prioritize MacBook while retaining Windows support.

[English](README.md) | [简体中文](README.zh-CN.md)

<!-- Keywords: sedentary reminder, sitting reminder, break reminder, stand up reminder, desk break, floating countdown timer, system tray app, 久坐提醒, Windows 11, Tauri, Rust -->

## Screenshots

These screenshots were captured on Windows. macOS shows its countdown in the top menu bar without a desktop floating timer; the panel and reminder use the same UI.

<p align="center">
  <img src="docs/screenshots/widget.png" alt="Floating countdown ball" width="140" />
</p>

<table>
  <tr>
    <td align="center"><img src="docs/screenshots/panel.png" alt="Tray panel" width="272" /></td>
    <td align="center"><img src="docs/screenshots/settings.png" alt="Settings window" width="320" /></td>
  </tr>
  <tr>
    <td align="center">Tray panel</td>
    <td align="center">Settings</td>
  </tr>
  <tr>
    <td align="center"><img src="docs/screenshots/reminder.png" alt="Break reminder" width="380" /></td>
    <td align="center"><img src="docs/screenshots/break.png" alt="Break countdown" width="380" /></td>
  </tr>
  <tr>
    <td align="center">Break reminder</td>
    <td align="center">Break countdown</td>
  </tr>
</table>

## Why Sit Break

- **Simple** — the Rust backend keeps time while panels are closed; hidden windows are reused
- **Clear** — a live menu-bar countdown on macOS; a glassmorphism floating ball and gradient progress ring on Windows
- **Smart** — system-wide idle detection: walk away for a coffee and the timer resets, so being away never counts as "sitting"
- **Bilingual** — Chinese & English UI, follows your system language or can be set manually

## Features

| | |
|---|---|
| ⏱ macOS menu-bar countdown | Shows `45:00` with live seconds, `⏸ 45:00` when paused and `Break 05:00` during a break; click to open the panel |
| ⏱ Windows floating countdown ball | Always-on-top, draggable, scroll-wheel resize (40–140 px), position remembered |
| 🔔 Break reminder popup | Glassmorphism card in the corner with a random stretch tip and a snooze option |
| 🧘 Break countdown | Circular progress ring, auto-resumes work timer when the break ends |
| ↻ One-click reset | Reset the countdown from the menu-bar or tray panel |
| 💤 Idle detection | Away for your break length (no keyboard/mouse) → timer resets automatically; can be turned off |
| 🚀 Launch at startup | Optional, one toggle in settings |
| 🔔 Sound cue | Gentle two-tone chime when a reminder pops up (optional) |
| ✏️ Custom tips | One per line, randomly selected for each reminder; consecutive repeats are avoided when different tips are available |
| 🌐 中文 / English | Follows system language, or pick one in settings |

## Download

Grab the latest installer or portable exe from [Releases](../../releases):

- `Sit Break_x.y.z_universal.dmg` — macOS installer for Apple Silicon and Intel; drag the app into Applications
- `Sit Break_x.y.z_x64-setup.exe` — NSIS installer
- `Sit Break_x.y.z_x64_en-US.msi` — MSI installer
- `sit-break.exe` — portable, no installation needed

> Requires macOS 12 Monterey or newer, or Windows 10/11 with WebView2.

## Build from Source

Prerequisites: [Node.js 22.12+](https://nodejs.org) and [Rust](https://rustup.rs). On macOS install Xcode Command Line Tools (`xcode-select --install`). On Windows install the [MSVC build tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) and WebView2.

After installing Rust, reopen your terminal or run `source "$HOME/.cargo/env"` on macOS.

```bash
npm ci
npm run tauri dev    # run in development
npm run tauri build  # macOS: .app + .dmg; Windows: exe + installers
```

Bundles are written to `src-tauri/target/release/bundle/`. For a local macOS app, use `npm run tauri build -- --bundles app`. To build a universal macOS release:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

The macOS app has no Dock icon or desktop floating timer. Its menu-bar countdown updates every second with monospaced digits to keep adjacent icons steady. Click the ring logo or countdown to open the panel; clicking outside dismisses it. Floating-timer controls are hidden in Mac settings. Reminders appear across desktops. Idle detection queries CoreGraphics session idle time on macOS and `GetLastInputInfo` on Windows.

Transparent windows use the macOS private API, with distribution through `.app` / `.dmg` and local ad-hoc signing by default. For public distribution, follow the [Tauri macOS signing guide](https://tauri.app/distribute/sign/macos/) to configure Developer ID signing and notarization. Developer ID credentials are not configured in the current workflow.

## How It Works

```
Work timer ──0──▶ Reminder popup ──Take a break──▶ Break countdown ──done──▶ Fresh timer
    ▲                                                                    │
    └────────────── Reset anytime (menu-bar / tray panel) ◀──────────────┘
```

- The timer, menu bar / tray, idle detection and window management live in **Rust**, independent of whether a frontend window is visible
- macOS creates three WebView windows (panel, settings and reminder); Windows also creates a floating-timer window. Windows are shown or hidden as the state changes

## Tech Stack

[Tauri 2](https://tauri.app) · Rust · TypeScript · Vite — no UI framework, hand-rolled CSS.

## CI & Releases

Regenerate the current desktop icons with `npm run icons`. See the [logo design](docs/design/logo-brief.md) for source assets and design choices.

- **CI**（`.github/workflows/ci.yml`）：frontend type-check/build, `cargo check`, `cargo clippy` and Rust tests on macOS and Windows
- **Release**（`.github/workflows/release.yml`）：pushing a `v*` tag builds the universal macOS DMG, Windows NSIS/MSI installers and portable exe

Cut a release with one command (syncs the npm, Tauri and Rust versions and lockfiles, commits, tags and pushes):

```bash
npm run release -- patch   # or minor / major / an explicit version like 1.2.3
# dry-run without touching files: npm run release -- patch --dry-run
```

## License

[MIT](LICENSE)
