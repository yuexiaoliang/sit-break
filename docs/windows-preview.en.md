# Windows Preview Builds

**English** | [简体中文](windows-preview.md)

When a pull request is updated, the [Windows preview workflow](https://github.com/yuexiaoliang/sit-break/actions/workflows/windows-preview.yml) builds installers for Windows 10/11 x64. Open the successful run for the desired commit and download `Sit-Break-Windows-x64-preview` from Artifacts. Downloads require a GitHub login. Files are retained for 14 days; the workflow does not create a formal release or version tag.

Extract the archive and run `Sit-Break-Preview-x64-setup.exe` to install, or use `Sit-Break-Preview-x64-portable.exe`. Quit other Sit Break instances first to avoid duplicate reminders. The installer can download WebView2 when needed; the portable version requires an existing WebView2 Runtime. Preview builds are not code-signed.

The app is named **Sit Break Preview**, with identifier `com.sitbreak.preview`. Settings and progress are stored under `%APPDATA%\com.sitbreak.preview`, separately from the regular release. The included Chinese and English instructions identify the source commit; `SHA256SUMS.txt` lets you verify the installer and executable.

Open settings through the system tray icon or floating timer. Choose Simple timer or Scheduled timer at the top of Work & breaks. In scheduled mode, use Edit schedules to configure periods and individual rounds, then save. To test reminders quickly, set both work and break lengths to 1 minute. Developer validation scope is recorded in the [schedule verification record](schedule-verification.md), maintained in Simplified Chinese.
