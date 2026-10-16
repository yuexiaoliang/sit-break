use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_autostart::MacosLauncher;
#[cfg(not(windows))]
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;

#[cfg(test)]
mod editing_tests;
#[cfg(test)]
mod guidance_resource_tests;
pub mod guidance_resources;
mod platform;
mod power;
mod window_geometry;
#[cfg(not(target_os = "macos"))]
use window_geometry::position_bottom_right;
use window_geometry::{clamp_axis, show_reminder};
pub mod preview;
#[cfg(test)]
mod preview_tests;
#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod recurrence_tests;
pub mod schedule;
#[cfg(test)]
mod sleep_tests;
#[cfg(test)]
mod startup_tests;
pub mod timer;
#[cfg(test)]
mod timer_tests;
use platform::idle_secs;
use timer::{Action, Mode, TimerApp, TimerEvent, TimerInfo, TimerOutput, TimerTime};

const POSTPONE_SECS: u64 = 5 * 60;

const TIPS: &[&str] = &[
    "站起来，踮脚 20 次，激活小腿血液循环",
    "抬头挺胸，双手向上伸展，深呼吸 5 次",
    "走动一下，顺便接杯水喝",
    "转转脖子：缓慢左右各看 5 秒",
    "靠墙站立 30 秒，收紧腹部和臀部",
    "伸展手臂到背后，扩胸 10 秒",
    "原地高抬腿 20 下，唤醒身体",
    "闭上眼睛休息 20 秒，缓解眼部疲劳",
    "扭扭腰，顺时针逆时针各转 5 圈",
    "下蹲 10 次，活动髋关节和膝盖",
];

fn default_tips() -> Vec<ReminderEntry> {
    TIPS.iter().map(|text| (*text).into()).collect()
}

const TIPS_EN: &[&str] = &[
    "Stand up and do 20 calf raises to get the blood flowing",
    "Stretch your arms overhead and take 5 deep breaths",
    "Walk around and refill your water bottle",
    "Roll your neck slowly: 5 seconds each side",
    "Stand against a wall for 30 seconds, core tight",
    "Clasp hands behind your back and open your chest for 10 seconds",
    "Do 20 high knees in place to wake your body up",
    "Close your eyes for 20 seconds to rest them",
    "Hip circles: 5 each direction",
    "Do 10 squats for your hips and knees",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReminderEntry {
    pub id: String,
    pub text: String,
    pub guidance: Option<Guidance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuidanceKind {
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum GuidanceSource {
    Local { id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guidance {
    pub kind: GuidanceKind,
    pub source: GuidanceSource,
}

impl Guidance {
    pub fn is_valid(&self) -> bool {
        let GuidanceSource::Local { id } = &self.source;
        guidance_resources::valid_id(id)
    }
}

impl From<String> for ReminderEntry {
    fn from(text: String) -> Self {
        Self {
            id: format!("reminder-{:032x}", fastrand::u128(..)),
            text,
            guidance: None,
        }
    }
}

impl From<&str> for ReminderEntry {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}

impl<'de> Deserialize<'de> for ReminderEntry {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StoredEntry {
            Legacy(String),
            Entry {
                id: String,
                text: String,
                #[serde(default)]
                guidance: Option<Guidance>,
            },
        }
        Ok(match StoredEntry::deserialize(deserializer)? {
            StoredEntry::Legacy(text) => text.into(),
            StoredEntry::Entry { id, text, guidance } => Self { id, text, guidance },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub work_minutes: u32,
    pub break_minutes: u32,
    pub use_schedule: bool,
    pub schedules: Vec<schedule::Schedule>,
    pub sound: bool,
    pub show_widget: bool,
    pub autostart: bool,
    pub idle_reset: bool,
    pub language: String,
    pub widget_size: u32,
    pub tips: Vec<ReminderEntry>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            work_minutes: 45,
            break_minutes: 5,
            use_schedule: false,
            schedules: Vec::new(),
            sound: true,
            show_widget: !cfg!(target_os = "macos"),
            autostart: false,
            idle_reset: true,
            language: "system".to_string(),
            widget_size: 64,
            tips: default_tips(),
        }
    }
}

impl Settings {
    pub fn load_from(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| {
                let mut stored: serde_json::Value = serde_json::from_str(&text).ok()?;
                // 清理开发测试版的 HTML 和其他来源，保留文字、日程和偏好。
                let remove_unsupported = |entry: &mut serde_json::Value| {
                    if entry.get("guidance").is_some_and(|guide| {
                        !guide.is_null()
                            && (guide["source"]["type"].as_str() != Some("local")
                                || guide["kind"].as_str() != Some("video"))
                    }) {
                        entry
                            .as_object_mut()
                            .map(|fields| fields.remove("guidance"));
                        return true;
                    }
                    false
                };
                if let Some(tips) = stored.get_mut("tips").and_then(|tips| tips.as_array_mut()) {
                    tips.retain_mut(|entry| {
                        !remove_unsupported(entry)
                            || entry["text"]
                                .as_str()
                                .is_some_and(|text| !text.trim().is_empty())
                    });
                }
                if let Some(schedules) = stored
                    .get_mut("schedules")
                    .and_then(|schedules| schedules.as_array_mut())
                {
                    for schedule in schedules {
                        if let Some(periods) = schedule
                            .get_mut("periods")
                            .and_then(|periods| periods.as_array_mut())
                        {
                            for period in periods {
                                if let Some(overrides) = period
                                    .get_mut("overrides")
                                    .and_then(|overrides| overrides.as_array_mut())
                                {
                                    for adjustment in overrides {
                                        remove_unsupported(adjustment);
                                    }
                                }
                            }
                        }
                    }
                }
                serde_json::from_value(stored).ok()
            })
            .unwrap_or_default()
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        let errors = self.validate();
        if !errors.is_empty() {
            return Err(serde_json::to_string(&errors).unwrap());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

/// true = 中文界面；language 为 "system" 时跟随系统区域
fn is_zh(settings: &Settings) -> bool {
    match settings.language.as_str() {
        "zh" => true,
        "en" => false,
        _ => sys_locale::get_locale()
            .map(|l| l.to_lowercase().starts_with("zh"))
            .unwrap_or(true),
    }
}

fn choose_tip(settings: &Settings, previous: Option<&str>, rng: &mut fastrand::Rng) -> String {
    let mut tips: Vec<&str> = settings
        .tips
        .iter()
        .map(|entry| entry.text.as_str())
        .filter(|tip| !tip.trim().is_empty())
        .collect();
    if tips.is_empty() {
        tips.extend_from_slice(if is_zh(settings) { TIPS } else { TIPS_EN });
    }

    // Exclude every copy of the previous text, so duplicate custom entries do
    // not cause consecutive repeats. A single distinct tip remains usable.
    let alternatives: Vec<&str> = tips
        .iter()
        .copied()
        .filter(|tip| Some(*tip) != previous)
        .collect();
    let candidates = if alternatives.is_empty() {
        &tips
    } else {
        &alternatives
    };
    candidates[rng.usize(..candidates.len())].to_string()
}

struct TimerClock(Instant);

struct GuidanceResourceState(Option<guidance_resources::GuidanceResources>);

#[derive(Default)]
struct PanelState {
    toggled_at: Mutex<Option<Instant>>,
    auto_hidden_at: Mutex<Option<Instant>>,
    opening: Mutex<PanelOpening>,
}

#[derive(Default)]
struct PanelOpening {
    ready: bool,
    pending: Option<Option<PhysicalPosition<f64>>>,
}

pub(crate) fn panel_builder<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    // 插件在窗口 setup 之前初始化，打开入口所需状态必须先注册。
    builder.manage(PanelState::default())
}

fn defer_panel_open<R: tauri::Runtime>(
    app: &AppHandle<R>,
    cursor: Option<PhysicalPosition<f64>>,
) -> bool {
    let state = app.state::<PanelState>();
    let mut opening = state.opening.lock().unwrap();
    if opening.ready {
        false
    } else {
        // 启动期间的多次打开合并为一次；None 光标同样表示有效打开请求。
        opening.pending = Some(cursor);
        true
    }
}

fn show_panel<R: tauri::Runtime>(
    app: &AppHandle<R>,
    cursor: Option<PhysicalPosition<f64>>,
) -> tauri::Result<()> {
    let panel = app
        .get_webview_window("panel")
        .ok_or(tauri::Error::WindowNotFound)?;
    *app.state::<PanelState>().toggled_at.lock().unwrap() = Some(Instant::now());
    match cursor {
        Some(c) => position_near_tray(&panel, c),
        None => position_panel_default(&panel),
    }
    panel.show()?;
    panel.set_focus()
}

/// 打开请求在初始化期间返回 false，窗口就绪后返回 true。
pub(crate) fn request_panel_open<R: tauri::Runtime>(
    app: &AppHandle<R>,
    cursor: Option<PhysicalPosition<f64>>,
) -> tauri::Result<bool> {
    if defer_panel_open(app, cursor) {
        return Ok(false);
    }
    // 再次启动表示展示已有实例，多次请求不会把已经显示的面板收起。
    show_panel(app, cursor)?;
    Ok(true)
}

/// 窗口和事件入口准备完成后，交付启动期间收到的打开请求。
pub(crate) fn finish_panel_setup<R: tauri::Runtime>(app: &AppHandle<R>) -> tauri::Result<bool> {
    if app.get_webview_window("panel").is_none() {
        return Err(tauri::Error::WindowNotFound);
    }
    let pending = {
        let state = app.state::<PanelState>();
        let mut opening = state.opening.lock().unwrap();
        opening.ready = true;
        opening.pending.take()
    };
    if let Some(cursor) = pending {
        show_panel(app, cursor)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn settings_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("settings.json"))
}

fn load_settings_from_disk(app: &AppHandle) -> Settings {
    settings_path(app)
        .map(|path| Settings::load_from(&path))
        .unwrap_or_default()
}

fn persist(app: &AppHandle, settings: &Settings) {
    if let Some(path) = settings_path(app) {
        let _ = settings.save_to(&path);
    }
}

fn progress_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    settings_path(app).map(|path| path.with_file_name("progress.json"))
}

fn persist_progress(app: &AppHandle) {
    let Some(timer) = app.try_state::<Mutex<TimerApp>>() else {
        return;
    };
    if let Some(path) = progress_path(app) {
        if let Err(error) = timer
            .lock()
            .unwrap()
            .save_progress_to(&path, timer_time(app))
        {
            eprintln!("无法保存运行进度：{error}");
        }
    }
}

// auto-launch 0.5.0 写 Run 键时不给路径加引号，安装目录 "Sit Break" 含空格会导致开机无法启动；
// Windows 下自己写注册表（路径加引号），其他平台仍走插件。
#[cfg(windows)]
fn autostart_set_reg(name: &str, exe: &std::path::Path, enable: bool) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
    if enable {
        let (key, _) = hkcu.create_subkey(path).map_err(|e| e.to_string())?;
        key.set_value(name, &format!("\"{}\"", exe.display()))
            .map_err(|e| e.to_string())?;
    } else {
        let key = match hkcu.open_subkey_with_flags(path, KEY_SET_VALUE) {
            Ok(key) => key,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if let Err(error) = key.delete_value(name) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error.to_string());
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn autostart_reg_is_enabled(name: &str) -> bool {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", KEY_READ)
        .ok()
        .and_then(|k| k.get_value::<String, _>(name).ok())
        .is_some()
}

#[cfg(windows)]
fn autostart_set(app: &AppHandle, enable: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    autostart_set_reg(&app.package_info().name, &exe, enable)
}

#[cfg(windows)]
fn autostart_is_enabled(app: &AppHandle) -> bool {
    autostart_reg_is_enabled(&app.package_info().name)
}

#[cfg(not(windows))]
fn autostart_set(app: &AppHandle, enable: bool) -> Result<(), String> {
    let al = app.autolaunch();
    let r = if enable { al.enable() } else { al.disable() };
    r.map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn autostart_is_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

fn position_panel_default<R: tauri::Runtime>(window: &WebviewWindow<R>) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let area = monitor.work_area();
    let ws = window.outer_size().unwrap_or_default();
    let margin = (16.0 * monitor.scale_factor()) as i32;
    let x = clamp_axis(i32::MAX, area.position.x, area.size.width, ws.width, margin);
    // macOS's menu bar is at the top; Windows's taskbar is usually at the bottom.
    let edge = if cfg!(target_os = "macos") {
        i32::MIN
    } else {
        i32::MAX
    };
    let y = clamp_axis(edge, area.position.y, area.size.height, ws.height, margin);
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn toggle_panel<R: tauri::Runtime>(app: &AppHandle<R>, cursor: Option<PhysicalPosition<f64>>) {
    if defer_panel_open(app, cursor) {
        return;
    }
    let panel_state = app.state::<PanelState>();
    *panel_state.toggled_at.lock().unwrap() = Some(Instant::now());
    // 面板显示时点击托盘会先触发失焦自动收起，紧接着才是 Click 事件；
    // 若刚被收起（就是本次点击所为），则视为已响应本次「隐藏」，不再重新弹出
    let recent_auto_hide = panel_state
        .auto_hidden_at
        .lock()
        .unwrap()
        .map(|t| t.elapsed() < Duration::from_millis(500))
        .unwrap_or(false);
    if let Some(panel) = app.get_webview_window("panel") {
        if panel.is_visible().unwrap_or(false) {
            let _ = panel.hide();
        } else if recent_auto_hide && cursor.is_some() {
            // 托盘点击：刚被本次点击收起，保持隐藏
        } else {
            match cursor {
                Some(c) => position_near_tray(&panel, c),
                None => position_panel_default(&panel),
            }
            let _ = panel.show();
            let _ = panel.set_focus();
        }
    }
}

fn position_near_tray<R: tauri::Runtime>(window: &WebviewWindow<R>, cursor: PhysicalPosition<f64>) {
    let ws = window.outer_size().unwrap_or_default();
    let monitor = window
        .app_handle()
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten();
    // 面板水平居中于点击点，垂直方向贴在光标上方
    let mut x = cursor.x as i32 - ws.width as i32 / 2;
    let mut y = cursor.y as i32 - ws.height as i32 - 8;
    if let Some(m) = &monitor {
        let area = m.work_area();
        let margin = (8.0 * m.scale_factor()) as i32;
        if y < area.position.y {
            // 上方放不下（如任务栏在顶部）→ 放到光标下方
            y = cursor.y as i32 + margin;
        }
        x = clamp_axis(x, area.position.x, area.size.width, ws.width, margin);
        y = clamp_axis(y, area.position.y, area.size.height, ws.height, margin);
    }
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn update_windows(app: &AppHandle) {
    let state = app.state::<Mutex<TimerApp>>();
    let st = state.lock().unwrap();
    let info = st.info();
    let mode = info.mode;
    let show_widget = st.settings().show_widget;
    drop(st);

    let widget = app.get_webview_window("widget");
    let reminder = app.get_webview_window("reminder");
    match mode {
        Mode::Work | Mode::Idle => {
            if let Some(w) = &widget {
                if show_widget {
                    let _ = w.show();
                } else {
                    let _ = w.hide();
                }
            }
            if let Some(r) = &reminder {
                let _ = r.hide();
            }
        }
        Mode::Remind | Mode::Break => {
            if let Some(w) = &widget {
                let _ = w.hide();
            }
            if let Some(r) = &reminder {
                let media = mode == Mode::Break && info.guidance.is_some();
                let _ = show_reminder(r, media);
            }
        }
    }
}

fn timer_time(app: &AppHandle) -> TimerTime {
    TimerTime {
        monotonic: app.state::<TimerClock>().0.elapsed(),
        local: Some(platform::local_time()),
    }
}

fn dispatch_timer(app: &AppHandle, action: Action) {
    let now = timer_time(app);
    let output = app
        .state::<Mutex<TimerApp>>()
        .lock()
        .unwrap()
        .dispatch(action, now);
    apply_timer_output(app, output);
}

fn apply_timer_output(app: &AppHandle, output: TimerOutput) {
    if let Some(settings) = &output.persist_settings {
        persist(app, settings);
    }
    if output.state.is_some() {
        persist_progress(app);
    }
    if output.update_windows {
        update_windows(app);
    }
    for event in output.events {
        match event {
            TimerEvent::Reminder(payload) => {
                let _ = app.emit("reminder", payload);
            }
            TimerEvent::BreakStarted { seconds } => {
                let _ = app.emit("break_started", json!({ "seconds": seconds }));
            }
        }
    }
    if let Some(info) = output.state {
        let _ = app.emit("tick", info);
    }
    if output.update_tray {
        update_tray(app);
    }
    if output.settings_changed {
        let _ = app.emit("settings_changed", ());
    }
}

fn fmt_mmss(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[cfg(any(target_os = "macos", test))]
fn menu_bar_title(mode: Mode, remaining: u64, paused: bool, zh: bool) -> String {
    let time = format!("{:02}:{:02}", remaining / 60, remaining % 60);
    let title = match mode {
        Mode::Work => time,
        Mode::Break => format!("{} {time}", if zh { "休息" } else { "Break" }),
        Mode::Remind => if zh { "起身" } else { "Move" }.to_string(),
        Mode::Idle => if zh { "待机" } else { "Idle" }.to_string(),
    };
    if paused {
        format!("⏸ {title}")
    } else {
        title
    }
}

fn update_tray(app: &AppHandle) {
    let st = app.state::<Mutex<TimerApp>>();
    let st = st.lock().unwrap();
    let info = st.info();
    #[cfg(target_os = "macos")]
    let title = {
        let status = menu_bar_title(info.mode, info.remaining, info.paused, is_zh(st.settings()));
        info.round.as_ref().map_or(status.clone(), |round| {
            format!(
                "{} {status}",
                if is_zh(st.settings()) {
                    format!("第{}轮", round.number)
                } else {
                    format!("R{}", round.number)
                }
            )
        })
    };
    let tooltip = match info.mode {
        _ if info.paused => {
            if is_zh(st.settings()) {
                format!("Sit Break · 已暂停（{}）", fmt_mmss(info.remaining))
            } else {
                format!("Sit Break · Paused ({})", fmt_mmss(info.remaining))
            }
        }
        Mode::Work => {
            if is_zh(st.settings()) {
                format!("Sit Break · 距离提醒还有 {}", fmt_mmss(info.remaining))
            } else {
                format!("Sit Break · Next break in {}", fmt_mmss(info.remaining))
            }
        }
        Mode::Break => {
            if is_zh(st.settings()) {
                format!("Sit Break · 休息中，剩余 {}", fmt_mmss(info.remaining))
            } else {
                format!("Sit Break · On a break, {} left", fmt_mmss(info.remaining))
            }
        }
        Mode::Remind => {
            if is_zh(st.settings()) {
                "Sit Break · 该起来活动了！".to_string()
            } else {
                "Sit Break · Time to move!".to_string()
            }
        }
        Mode::Idle => {
            if is_zh(st.settings()) {
                "Sit Break · 待机，现在不提醒".to_string()
            } else {
                "Sit Break · Idle, no reminders now".to_string()
            }
        }
    };
    let tooltip = if let Some(round) = &info.round {
        if is_zh(st.settings()) {
            format!(
                "{tooltip} · 第 {} 轮 · 工作 {}/休息 {} 分钟",
                round.number,
                info.work_seconds / 60,
                info.break_seconds / 60
            )
        } else {
            format!(
                "{tooltip} · Round {} · Work {}/break {} min",
                round.number,
                info.work_seconds / 60,
                info.break_seconds / 60
            )
        }
    } else {
        tooltip
    };
    drop(st);
    if let Some(tray) = app.tray_by_id("main-tray") {
        #[cfg(target_os = "macos")]
        let _ = tray.set_title(Some(title));
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

#[tauri::command]
fn apply_interval(app: AppHandle, minutes: u32) {
    dispatch_timer(&app, Action::ApplyInterval(minutes));
}

#[tauri::command]
fn apply_break(app: AppHandle, minutes: u32) {
    dispatch_timer(&app, Action::ApplyBreak(minutes));
}

// ---------------- Tauri commands ----------------

#[tauri::command]
async fn import_guidance_video(app: AppHandle) -> Result<Option<Guidance>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resources = app.state::<GuidanceResourceState>();
        let resources = resources.0.as_ref().ok_or("resource-unavailable")?;
        let title = if is_zh(app.state::<Mutex<TimerApp>>().lock().unwrap().settings()) {
            "导入本地视频"
        } else {
            "Import local video"
        };
        let mut dialog = app
            .dialog()
            .file()
            .set_title(title)
            .add_filter("MP4 video", &["mp4", "m4v"]);
        if let Some(window) = app.get_webview_window("settings") {
            dialog = dialog.set_parent(&window);
        }
        let Some(file) = dialog.blocking_pick_file() else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|_| "import-failed")?;
        resources.import_video(&path).map(Some)
    })
    .await
    .map_err(|_| "import-failed".to_string())?
}

#[tauri::command]
fn get_platform() -> &'static str {
    std::env::consts::OS
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    app.state::<Mutex<TimerApp>>()
        .lock()
        .unwrap()
        .settings()
        .clone()
}

#[tauri::command]
fn timer_info(app: AppHandle) -> TimerInfo {
    app.state::<Mutex<TimerApp>>().lock().unwrap().info()
}

#[tauri::command]
fn fit_reminder_height(window: WebviewWindow, app: AppHandle, height: u32) -> Result<(), String> {
    let info = app.state::<Mutex<TimerApp>>().lock().unwrap().info();
    if window.label() == "reminder" && info.mode == Mode::Break && info.guidance.is_none() {
        window_geometry::fit_text_break(&window, height).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn preview_schedule(
    app: AppHandle,
    draft: Settings,
) -> Result<preview::SchedulePreview, serde_json::Value> {
    let now = timer_time(&app);
    app.state::<Mutex<TimerApp>>()
        .lock()
        .unwrap()
        .preview(&draft, now)
        .map_err(|errors| json!({ "kind": "validation", "errors": errors }))
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), serde_json::Value> {
    let errors = settings.validate();
    if !errors.is_empty() {
        return Err(json!({ "kind": "validation", "errors": errors }));
    }
    if guidance_resources::settings_guidance(&settings)
        .next()
        .is_some()
    {
        let state = app.state::<GuidanceResourceState>();
        let resources = state.0.as_ref().ok_or_else(|| json!({"kind":"resource"}))?;
        resources
            .validate_settings(&settings)
            .map_err(|_| json!({"kind":"resource"}))?;
    }
    if settings.autostart != autostart_is_enabled(&app) {
        autostart_set(&app, settings.autostart)
            .map_err(|message| json!({ "kind": "system", "message": message }))?;
    }
    let widget_size = settings.widget_size.clamp(40, 140);
    let settings = Settings {
        widget_size,
        ..settings
    };

    dispatch_timer(&app, Action::SaveSettings(settings));
    Ok(())
}

#[tauri::command]
fn start_break(app: AppHandle) {
    dispatch_timer(&app, Action::StartBreak);
}

#[tauri::command]
fn postpone(app: AppHandle) {
    dispatch_timer(&app, Action::Postpone);
}

#[tauri::command]
fn end_break(app: AppHandle) {
    dispatch_timer(&app, Action::EndBreak);
}

#[tauri::command]
fn reset_timer(app: AppHandle) {
    dispatch_timer(&app, Action::Reset);
}

#[tauri::command]
fn toggle_pause(app: AppHandle) {
    dispatch_timer(&app, Action::TogglePause);
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = app.emit("settings_open", ());
    }
}

#[tauri::command]
fn hide_settings(app: AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.hide();
    }
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    persist_progress(&app);
    app.exit(0);
}

// ---------------- timer thread ----------------

fn run_timer(app: AppHandle, power_events: mpsc::Receiver<power::PowerEvent>) {
    std::thread::spawn(move || {
        let mut last_minute = u64::MAX;
        #[cfg(windows)]
        let mut tray_light_theme = None;
        loop {
            // 原生唤醒结束等待；每次输入后重新等一秒，普通 tick 不再重复
            // 累计已由 Resume 处理的睡眠区间。
            let action = match power_events.recv_timeout(Duration::from_secs(1)) {
                Ok(power::PowerEvent::Suspend { idle_secs }) => Action::Suspend { idle_secs },
                Ok(power::PowerEvent::Resume {
                    elapsed_secs,
                    idle_secs,
                }) => Action::Resume {
                    elapsed_secs,
                    idle_secs,
                },
                Err(mpsc::RecvTimeoutError::Timeout) => Action::Tick {
                    elapsed_secs: 1,
                    idle_secs: idle_secs(),
                },
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            // The taskbar follows system mode independently of the app theme.
            #[cfg(windows)]
            if let Some(light_theme) = platform::system_uses_light_theme() {
                if tray_light_theme != Some(light_theme) {
                    if let Some(tray) = app.tray_by_id("main-tray") {
                        if tray
                            .set_icon(Some(monochrome_tray_icon(light_theme)))
                            .is_ok()
                        {
                            tray_light_theme = Some(light_theme);
                        }
                    }
                }
            }
            let now = timer_time(&app);
            let mut output = app
                .state::<Mutex<TimerApp>>()
                .lock()
                .unwrap()
                .dispatch(action, now);
            // 菜单栏每秒更新；Windows 工作中的 tooltip 仍按分钟更新。
            if let Some(info) = &output.state {
                let minute = info.remaining / 60;
                output.update_tray = cfg!(target_os = "macos")
                    || info.mode != Mode::Work
                    || output.update_windows
                    || minute != last_minute;
                last_minute = minute;
            }
            apply_timer_output(&app, output);
        }
    });
}

// ---------------- app entry ----------------

fn native_check_window<'a, R: tauri::Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    // 提升权限的 Windows CI 可能忽略 WebView2 环境参数；发布构建不包含此入口。
    #[cfg(all(windows, debug_assertions))]
    if let Some(port) = std::env::var("SIT_BREAK_NATIVE_CHECK_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port > 0)
    {
        return builder.additional_browser_args(&format!(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-address=127.0.0.1 --remote-debugging-port={port}"
        ));
    }
    builder
}

#[cfg(not(target_os = "macos"))]
fn widget_window_size(size: u32) -> f64 {
    let size = size as f64 + 24.0;
    if cfg!(windows) {
        size.max(140.0)
    } else {
        size
    }
}

#[cfg(any(target_os = "macos", windows))]
fn monochrome_tray_icon(use_black: bool) -> tauri::image::Image<'static> {
    let image = tauri::include_image!("./icons/tray-icon.png");
    if use_black {
        return image;
    }
    let mut pixels = image.rgba().to_vec();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel[..3].fill(255);
    }
    tauri::image::Image::new_owned(pixels, image.width(), image.height())
}

#[cfg(target_os = "macos")]
fn tray_icon(_app: &AppHandle) -> tauri::image::Image<'static> {
    // The transparent, 2x ring template is automatically tinted by AppKit.
    monochrome_tray_icon(true)
}

#[cfg(windows)]
fn tray_icon(app: &AppHandle) -> tauri::image::Image<'static> {
    match platform::system_uses_light_theme() {
        Some(light_theme) => monochrome_tray_icon(light_theme),
        None => owned_window_icon(app.default_window_icon().unwrap()),
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn tray_icon(app: &AppHandle) -> tauri::image::Image<'static> {
    owned_window_icon(app.default_window_icon().unwrap())
}

#[cfg(any(not(target_os = "macos"), test))]
fn owned_window_icon(image: &tauri::image::Image<'_>) -> tauri::image::Image<'static> {
    // Detach the pixels from the borrowed app icon before handing them to the tray.
    image.clone().to_owned()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (power_sender, power_receiver) = mpsc::channel();
    let app = panel_builder(tauri::Builder::default())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 已有实例在运行：把它唤醒（弹出面板），本进程立即退出
            if let Err(error) = request_panel_open(app, None) {
                eprintln!("error opening existing application: {error}");
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("guidance", |context, request| {
            // 用户网页的 opaque Origin 不可读取副本。播放器的普通 no-cors 请求仍可播放。
            let trusted_origin = request
                .headers()
                .get("origin")
                .filter(|origin| {
                    let Ok(origin) = origin.to_str() else {
                        return false;
                    };
                    matches!(
                        origin,
                        "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
                    ) || (cfg!(debug_assertions)
                        && context
                            .app_handle()
                            .config()
                            .build
                            .dev_url
                            .as_ref()
                            .is_some_and(|url| url.origin().ascii_serialization() == origin))
                })
                .cloned();
            let state = context.app_handle().state::<GuidanceResourceState>();
            let mut response = match &state.0 {
                Some(resources) => resources.video_response(request),
                None => tauri::http::Response::builder()
                    .status(503)
                    .body(Vec::new())
                    .unwrap(),
            };
            response
                .headers_mut()
                .insert("vary", tauri::http::HeaderValue::from_static("Origin"));
            if let Some(origin) = trusted_origin {
                response
                    .headers_mut()
                    .insert("access-control-allow-origin", origin);
            }
            response
        })
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            get_platform,
            import_guidance_video,
            get_settings,
            timer_info,
            fit_reminder_height,
            preview_schedule,
            save_settings,
            apply_interval,
            apply_break,
            start_break,
            postpone,
            end_break,
            reset_timer,
            toggle_pause,
            open_settings,
            hide_settings,
            quit_app
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            let mut settings = load_settings_from_disk(&handle);
            let autostart_enabled = autostart_is_enabled(&handle);
            settings.autostart = autostart_enabled;
            handle.manage(TimerClock(Instant::now()));
            let now = timer_time(&handle);
            let timer = if let Some(path) = progress_path(&handle) {
                TimerApp::restore_from(settings.clone(), &path, fastrand::Rng::new(), now)
            } else {
                TimerApp::new_at(settings.clone(), fastrand::Rng::new(), now)
            };
            let resources = handle.path().app_data_dir().ok().and_then(|directory| {
                guidance_resources::GuidanceResources::open(directory.join("guidance")).ok()
            });
            if let Some(resources) = &resources {
                if let Err(error) = resources.retain_for(timer.settings(), &timer.info()) {
                    eprintln!("无法回收未引用的动作指引：{error}");
                }
            }
            handle.manage(GuidanceResourceState(resources));
            handle.manage(Mutex::new(timer));

            // ---- 窗口（在 manage 之后创建，页面加载时状态已就绪）----
            #[cfg(not(target_os = "macos"))]
            let widget = native_check_window(WebviewWindowBuilder::new(
                &handle,
                "widget",
                WebviewUrl::App("widget.html".into()),
            ))
            .title("Sit Break")
            .inner_size(
                widget_window_size(settings.widget_size),
                widget_window_size(settings.widget_size),
            )
            .visible(false)
            .decorations(false)
            .transparent(true)
            .accept_first_mouse(true)
            .shadow(false)
            .always_on_top(true)
            .visible_on_all_workspaces(cfg!(target_os = "macos"))
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .build()?;
            let zh = is_zh(&settings);
            let reminder = native_check_window(WebviewWindowBuilder::new(
                &handle,
                "reminder",
                WebviewUrl::App("reminder.html".into()),
            ))
            .title(if zh {
                "Sit Break 提醒"
            } else {
                "Sit Break Reminder"
            })
            .inner_size(380.0, 368.0)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .accept_first_mouse(true)
            .shadow(false)
            .always_on_top(true)
            .visible_on_all_workspaces(cfg!(target_os = "macos"))
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .build()?;
            let settings_win = native_check_window(WebviewWindowBuilder::new(
                &handle,
                "settings",
                WebviewUrl::App("settings.html".into()),
            ))
            .title(if zh {
                "Sit Break 设置"
            } else {
                "Sit Break Settings"
            })
            .inner_size(562.0, 785.0)
            .min_inner_size(320.0, 420.0)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .accept_first_mouse(true)
            .shadow(false)
            .skip_taskbar(true)
            .resizable(true)
            .center()
            .focused(false)
            .build()?;
            let panel = native_check_window(WebviewWindowBuilder::new(
                &handle,
                "panel",
                WebviewUrl::App("panel.html".into()),
            ))
            .title("Sit Break")
            .inner_size(272.0, 340.0)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .accept_first_mouse(true)
            .shadow(false)
            .always_on_top(true)
            .visible_on_all_workspaces(cfg!(target_os = "macos"))
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .build()?;

            #[cfg(target_os = "macos")]
            for window in [&reminder, &panel] {
                platform::configure_overlay(window)?;
            }

            // ---- 托盘（左/右键均弹出面板，无原生菜单）----
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon(&handle))
                .icon_as_template(cfg!(target_os = "macos"))
                .tooltip("Sit Break")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    // Windows 下一次点击会先后触发 Down/Up 两个事件，只在 Up 时切换，
                    // 否则一次点击会 toggle 两次导致面板闪现后消失
                    if let tauri::tray::TrayIconEvent::Click {
                        position,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        // 直接用点击时的光标位置定位，图标 rect 在部分场景（任务栏溢出区等）会不准
                        toggle_panel(tray.app_handle(), Some(position));
                    }
                })
                .build(&handle)?;

            #[cfg(target_os = "macos")]
            platform::configure_menu_bar(&_tray)?;

            // ---- 窗口事件：关闭一律隐藏 ----
            reminder.on_window_event({
                let rw = reminder.clone();
                move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = rw.hide();
                        // 关闭待开始的提醒视同「再等等」；关闭已开始的休息先结束，
                        // 防止隐藏的浮窗继续播放用户已经打开声音的视频。
                        let app = rw.app_handle();
                        let mode = app.state::<Mutex<TimerApp>>().lock().unwrap().info().mode;
                        if mode == Mode::Break {
                            end_break(app.clone());
                        } else {
                            postpone(app.clone());
                        }
                    }
                }
            });
            settings_win.on_window_event({
                let sw = settings_win.clone();
                move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = sw.hide();
                    }
                }
            });
            #[cfg(not(target_os = "macos"))]
            widget.on_window_event({
                let wh = widget.clone();
                move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = wh.hide();
                    }
                }
            });
            panel.on_window_event({
                let panel = panel.clone();
                move |event| match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = panel.hide();
                    }
                    tauri::WindowEvent::Focused(false) => {
                        // 点击面板以外的地方自动收起；托盘开/关动作本身除外
                        let app = panel.app_handle();
                        let recent = app
                            .state::<PanelState>()
                            .toggled_at
                            .lock()
                            .unwrap()
                            .map(|t| t.elapsed() < Duration::from_millis(500))
                            .unwrap_or(false);
                        if !recent {
                            let _ = panel.hide();
                            // 记录时间，供托盘 Click 判断「是否刚被本次点击收起」
                            *app.state::<PanelState>().auto_hidden_at.lock().unwrap() =
                                Some(Instant::now());
                        }
                    }
                    _ => {}
                }
            });

            update_tray(&handle);
            #[cfg(not(target_os = "macos"))]
            if settings.show_widget {
                position_bottom_right(&widget);
            }
            update_windows(&handle);
            finish_panel_setup(&handle)?;
            run_timer(handle.clone(), power_receiver);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    let mut power_monitor = Some(
        power::PowerMonitor::new(power_sender).expect("error registering system sleep monitor"),
    );
    app.run(move |handle, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            persist_progress(handle);
        }
        if matches!(event, tauri::RunEvent::Exit) {
            power_monitor.take();
        }
    });
}

#[cfg(test)]
mod tray_icon_tests {
    use super::owned_window_icon;

    #[test]
    fn owned_icon_preserves_pixels_after_source_is_dropped() {
        let icon = {
            let rgba = vec![255, 255, 255, 255, 0, 0, 0, 0];
            let borrowed = tauri::image::Image::new(&rgba, 2, 1);
            owned_window_icon(&borrowed)
        };
        assert_eq!((icon.width(), icon.height()), (2, 1));
        assert_eq!(icon.rgba(), &[255, 255, 255, 255, 0, 0, 0, 0]);
    }
}

#[cfg(test)]
mod tip_tests {
    use super::{choose_tip, Settings, TIPS, TIPS_EN};
    use std::collections::HashSet;

    #[test]
    fn consecutive_reminders_do_not_repeat_the_previous_tip() {
        let settings = Settings::default();
        let mut rng = fastrand::Rng::with_seed(42);
        let mut previous = TIPS[0].to_string();
        for _ in 0..100 {
            let tip = choose_tip(&settings, Some(&previous), &mut rng);
            assert_ne!(tip, previous);
            assert!(settings.tips.iter().any(|entry| entry.text == tip));
            previous = tip;
        }
    }

    #[test]
    fn custom_tips_beyond_the_first_ten_can_be_selected() {
        let settings = Settings {
            tips: (0..25).map(|i| format!("Custom tip {i}").into()).collect(),
            ..Settings::default()
        };
        let mut rng = fastrand::Rng::with_seed(7);
        let mut seen = HashSet::new();
        let mut previous: Option<String> = None;
        for _ in 0..1000 {
            let tip = choose_tip(&settings, previous.as_deref(), &mut rng);
            assert!(settings.tips.iter().any(|entry| entry.text == tip));
            seen.insert(tip.clone());
            previous = Some(tip);
        }
        assert_eq!(seen.len(), settings.tips.len());
    }

    #[test]
    fn empty_or_blank_custom_tips_use_defaults_in_the_selected_language() {
        let mut settings = Settings {
            tips: vec!["  ".into(), "\n".into()],
            language: "en".into(),
            ..Settings::default()
        };
        let mut rng = fastrand::Rng::with_seed(1);
        assert!(TIPS_EN.contains(&choose_tip(&settings, None, &mut rng).as_str()));
        settings.language = "zh".into();
        settings.tips.clear();
        assert!(TIPS.contains(&choose_tip(&settings, None, &mut rng).as_str()));
    }

    #[test]
    fn a_single_distinct_custom_tip_remains_usable() {
        let settings = Settings {
            tips: vec!["Only tip".into(), "Only tip".into()],
            ..Settings::default()
        };
        let mut rng = fastrand::Rng::with_seed(1);
        assert_eq!(choose_tip(&settings, None, &mut rng), "Only tip");
        assert_eq!(
            choose_tip(&settings, Some("Only tip"), &mut rng),
            "Only tip"
        );
    }

    #[test]
    fn duplicate_text_is_excluded_when_an_alternative_exists() {
        let settings = Settings {
            tips: vec!["Same".into(), "Same".into(), "Different".into()],
            ..Settings::default()
        };
        let mut rng = fastrand::Rng::with_seed(1);
        assert_eq!(choose_tip(&settings, Some("Same"), &mut rng), "Different");
    }

    #[test]
    fn editing_the_tip_list_does_not_reuse_old_entries() {
        let mut settings = Settings {
            tips: vec!["Old tip".into()],
            ..Settings::default()
        };
        let mut rng = fastrand::Rng::with_seed(1);
        let previous = choose_tip(&settings, None, &mut rng);
        settings.tips = vec!["New tip".into(), "Another new tip".into()];
        let tip = choose_tip(&settings, Some(&previous), &mut rng);
        assert!(settings.tips.iter().any(|entry| entry.text == tip));
    }
}

#[cfg(test)]
mod menu_bar_tests {
    use super::{menu_bar_title, Mode};

    #[test]
    fn working_countdown_shows_seconds_and_padded_minutes() {
        assert_eq!(menu_bar_title(Mode::Work, 2700, false, true), "45:00");
        assert_eq!(menu_bar_title(Mode::Work, 2699, false, true), "44:59");
        assert_eq!(menu_bar_title(Mode::Work, 59, false, false), "00:59");
        assert_eq!(menu_bar_title(Mode::Work, 14400, false, false), "240:00");
    }

    #[test]
    fn pause_is_visible_during_work_and_breaks() {
        assert_eq!(menu_bar_title(Mode::Work, 120, true, true), "⏸ 02:00");
        assert_eq!(
            menu_bar_title(Mode::Break, 120, true, false),
            "⏸ Break 02:00"
        );
    }

    #[test]
    fn break_and_reminder_status_follow_the_selected_language() {
        assert_eq!(menu_bar_title(Mode::Break, 285, false, true), "休息 04:45");
        assert_eq!(
            menu_bar_title(Mode::Break, 285, false, false),
            "Break 04:45"
        );
        assert_eq!(menu_bar_title(Mode::Remind, 0, false, true), "起身");
        assert_eq!(menu_bar_title(Mode::Remind, 0, false, false), "Move");
    }
}

#[cfg(test)]
mod window_position_tests {
    use super::clamp_axis;

    #[test]
    fn bottom_right_respects_secondary_monitor_origin() {
        assert_eq!(clamp_axis(i32::MAX, -1920, 1920, 380, 24), -404);
        assert_eq!(clamp_axis(i32::MAX, 1080, 1080, 368, 24), 1768);
    }

    #[test]
    fn panel_stays_below_menu_bar_and_inside_work_area() {
        assert_eq!(clamp_axis(-320, 48, 1700, 680, 16), 64);
        assert_eq!(clamp_axis(2000, 48, 1700, 680, 16), 1052);
    }

    #[test]
    fn small_work_areas_do_not_produce_invalid_clamp_bounds() {
        assert_eq!(clamp_axis(1000, -100, 300, 380, 24), -100);
        assert_eq!(clamp_axis(1000, 100, 390, 380, 24), 105);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn autostart_registry_roundtrip() {
        let exe = std::env::current_exe().unwrap();
        let name = "Sit Break Test";
        autostart_set_reg(name, &exe, false).unwrap();
        assert!(!autostart_reg_is_enabled(name));
        autostart_set_reg(name, &exe, true).unwrap();
        assert!(autostart_reg_is_enabled(name));
        // 路径必须带引号，否则安装目录 "Sit Break" 含空格会导致开机无法启动
        {
            use winreg::enums::*;
            let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
            let v: String = hkcu
                .open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", KEY_READ)
                .unwrap()
                .get_value(name)
                .unwrap();
            assert!(
                v.starts_with('"') && v.ends_with('"'),
                "value not quoted: {v}"
            );
        }
        autostart_set_reg(name, &exe, false).unwrap();
        assert!(!autostart_reg_is_enabled(name));
        autostart_set_reg(name, &exe, false).unwrap();
    }
}
