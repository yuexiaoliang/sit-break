use std::sync::Mutex;
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

mod platform;
use platform::idle_secs;

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

fn default_tips() -> Vec<String> {
    TIPS.iter().map(|s| s.to_string()).collect()
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

fn default_tips_en() -> Vec<String> {
    TIPS_EN.iter().map(|s| s.to_string()).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    work_minutes: u32,
    break_minutes: u32,
    sound: bool,
    show_widget: bool,
    autostart: bool,
    idle_reset: bool,
    language: String,
    widget_size: u32,
    tips: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            work_minutes: 45,
            break_minutes: 5,
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
        .map(|tip| tip.trim())
        .filter(|tip| !tip.is_empty())
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Work,
    Remind,
    Break,
}

struct AppState {
    mode: Mode,
    remaining: u64,
    paused: bool,
    break_ended_at: Option<Instant>,
    settings: Settings,
}

struct PanelState {
    toggled_at: Mutex<Option<Instant>>,
    auto_hidden_at: Mutex<Option<Instant>>,
}

fn settings_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("settings.json"))
}

fn load_settings_from_disk(app: &AppHandle) -> Settings {
    settings_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn persist(app: &AppHandle, settings: &Settings) {
    if let Some(path) = settings_path(app) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(settings) {
            let _ = std::fs::write(path, text);
        }
    }
}

/// 空闲自动重置的阈值（秒）：与设置的休息时长一致；关闭该功能时返回 u64::MAX 永不触发
fn idle_reset_limit_secs(settings: &Settings) -> u64 {
    if settings.idle_reset {
        settings.break_minutes as u64 * 60
    } else {
        u64::MAX
    }
}

// auto-launch 0.5.0 写 Run 键时不给路径加引号，安装目录 "Sit Break" 含空格会导致开机无法启动；
// Windows 下自己写注册表（路径加引号），其他平台仍走插件。
#[cfg(windows)]
fn autostart_set_reg(name: &str, exe: &std::path::Path, enable: bool) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE,
        )
        .map_err(|e| e.to_string())?;
    if enable {
        key.set_value(name, &format!("\"{}\"", exe.display()))
            .map_err(|e| e.to_string())?;
    } else {
        let _ = key.delete_value(name);
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

fn position_bottom_right(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let area = monitor.work_area();
    let ws = window.outer_size().unwrap_or_default();
    let margin = (24.0 * monitor.scale_factor()) as i32;
    let x = clamp_axis(i32::MAX, area.position.x, area.size.width, ws.width, margin);
    let y = clamp_axis(
        i32::MAX,
        area.position.y,
        area.size.height,
        ws.height,
        margin,
    );
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn clamp_axis(candidate: i32, origin: i32, available: u32, window: u32, margin: i32) -> i32 {
    if window >= available {
        return origin;
    }
    let margin = margin.min((available - window) as i32 / 2).max(0);
    candidate.clamp(
        origin + margin,
        origin + available as i32 - window as i32 - margin,
    )
}

fn position_panel_default(window: &WebviewWindow) {
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

fn toggle_panel(app: &AppHandle, cursor: Option<PhysicalPosition<f64>>) {
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

fn position_near_tray(window: &WebviewWindow, cursor: PhysicalPosition<f64>) {
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
    let state = app.state::<Mutex<AppState>>();
    let st = state.lock().unwrap();
    let mode = st.mode;
    let show_widget = st.settings.show_widget;
    drop(st);

    let widget = app.get_webview_window("widget");
    let reminder = app.get_webview_window("reminder");
    match mode {
        Mode::Work => {
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
                position_bottom_right(r);
                let _ = r.show();
            }
        }
    }
}

fn emit_state(app: &AppHandle) {
    let state = app.state::<Mutex<AppState>>();
    let st = state.lock().unwrap();
    let payload = json!({
        "mode": match st.mode { Mode::Work => "work", Mode::Remind => "remind", Mode::Break => "break" },
        "remaining": st.remaining,
        "paused": st.paused,
    });
    drop(st);
    let _ = app.emit("tick", payload);
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
    };
    if paused {
        format!("⏸ {title}")
    } else {
        title
    }
}

fn update_tray(app: &AppHandle) {
    let st = app.state::<Mutex<AppState>>();
    let st = st.lock().unwrap();
    #[cfg(target_os = "macos")]
    let title = menu_bar_title(st.mode, st.remaining, st.paused, is_zh(&st.settings));
    let tooltip = match st.mode {
        _ if st.paused => {
            if is_zh(&st.settings) {
                format!("Sit Break · 已暂停（{}）", fmt_mmss(st.remaining))
            } else {
                format!("Sit Break · Paused ({})", fmt_mmss(st.remaining))
            }
        }
        Mode::Work => {
            if is_zh(&st.settings) {
                format!("Sit Break · 距离提醒还有 {}", fmt_mmss(st.remaining))
            } else {
                format!("Sit Break · Next break in {}", fmt_mmss(st.remaining))
            }
        }
        Mode::Break => {
            if is_zh(&st.settings) {
                format!("Sit Break · 休息中，剩余 {}", fmt_mmss(st.remaining))
            } else {
                format!("Sit Break · On a break, {} left", fmt_mmss(st.remaining))
            }
        }
        Mode::Remind => {
            if is_zh(&st.settings) {
                "Sit Break · 该起来活动了！".to_string()
            } else {
                "Sit Break · Time to move!".to_string()
            }
        }
    };
    drop(st);
    if let Some(tray) = app.tray_by_id("main-tray") {
        #[cfg(target_os = "macos")]
        let _ = tray.set_title(Some(title));
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

fn back_to_work(app: &AppHandle) {
    let state = app.state::<Mutex<AppState>>();
    let mut st = state.lock().unwrap();
    st.mode = Mode::Work;
    st.remaining = st.settings.work_minutes as u64 * 60;
    st.break_ended_at = Some(Instant::now());
    drop(st);
    update_windows(app);
    emit_state(app);
    update_tray(app);
}

#[tauri::command]
fn apply_interval(app: AppHandle, minutes: u32) {
    let state = app.state::<Mutex<AppState>>();
    let mut st = state.lock().unwrap();
    st.settings.work_minutes = minutes;
    if st.mode == Mode::Work {
        st.remaining = minutes as u64 * 60;
    }
    drop(st);
    persist(
        &app,
        &app.state::<Mutex<AppState>>().lock().unwrap().settings,
    );
    emit_state(&app);
    update_tray(&app);
    let _ = app.emit("settings_changed", ());
}

#[tauri::command]
fn apply_break(app: AppHandle, minutes: u32) {
    let state = app.state::<Mutex<AppState>>();
    let mut st = state.lock().unwrap();
    st.settings.break_minutes = minutes;
    drop(st);
    persist(
        &app,
        &app.state::<Mutex<AppState>>().lock().unwrap().settings,
    );
    let _ = app.emit("settings_changed", ());
}

// ---------------- Tauri commands ----------------

#[tauri::command]
fn get_platform() -> &'static str {
    std::env::consts::OS
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    app.state::<Mutex<AppState>>()
        .lock()
        .unwrap()
        .settings
        .clone()
}

#[tauri::command]
fn timer_info(app: AppHandle) -> serde_json::Value {
    let st = app.state::<Mutex<AppState>>();
    let st = st.lock().unwrap();
    json!({
        "mode": match st.mode { Mode::Work => "work", Mode::Remind => "remind", Mode::Break => "break" },
        "remaining": st.remaining,
        "paused": st.paused,
    })
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    if settings.autostart != autostart_is_enabled(&app) {
        autostart_set(&app, settings.autostart)?;
    }
    let widget_size = settings.widget_size.clamp(40, 140);
    let mut settings = Settings {
        widget_size,
        ..settings
    };

    // 切换语言时，若提示列表仍是另一语言的默认文案，则跟随切换（不覆盖自定义内容）
    let zh_now = is_zh(&settings);
    let other_defaults = if zh_now {
        default_tips_en()
    } else {
        default_tips()
    };
    if settings.tips == other_defaults {
        settings.tips = if zh_now {
            default_tips()
        } else {
            default_tips_en()
        };
    }

    let was_showing_widget;
    let interval_applied;
    {
        let state = app.state::<Mutex<AppState>>();
        let mut st = state.lock().unwrap();
        was_showing_widget = st.settings.show_widget;
        interval_applied = settings.work_minutes != st.settings.work_minutes;
        st.settings = settings.clone();
        st.settings.widget_size = widget_size;
        if st.mode == Mode::Work {
            let interval = settings.work_minutes as u64 * 60;
            if interval_applied || st.remaining > interval {
                st.remaining = interval;
            }
        }
    }
    persist(&app, &settings);

    if was_showing_widget != settings.show_widget {
        update_windows(&app);
    }
    emit_state(&app);
    update_tray(&app);
    let _ = app.emit("settings_changed", ());
    Ok(())
}

#[tauri::command]
fn start_break(app: AppHandle) {
    let state = app.state::<Mutex<AppState>>();
    {
        let mut st = state.lock().unwrap();
        if st.mode != Mode::Remind {
            return;
        }
        st.mode = Mode::Break;
        st.remaining = st.settings.break_minutes as u64 * 60;
    }
    let _ = app.emit(
        "break_started",
        json!({ "seconds": app.state::<Mutex<AppState>>().lock().unwrap().remaining }),
    );
    update_tray(&app);
}

#[tauri::command]
fn postpone(app: AppHandle) {
    let state = app.state::<Mutex<AppState>>();
    {
        let mut st = state.lock().unwrap();
        if st.mode != Mode::Remind {
            return;
        }
        st.mode = Mode::Work;
        st.remaining = POSTPONE_SECS
            .min(st.settings.work_minutes as u64 * 60)
            .max(60);
    }
    update_windows(&app);
    emit_state(&app);
    update_tray(&app);
}

#[tauri::command]
fn end_break(app: AppHandle) {
    let state = app.state::<Mutex<AppState>>();
    {
        let st = state.lock().unwrap();
        if st.mode != Mode::Break {
            return;
        }
    }
    back_to_work(&app);
}

#[tauri::command]
fn reset_timer(app: AppHandle) {
    let state = app.state::<Mutex<AppState>>();
    let mut st = state.lock().unwrap();
    if st.mode != Mode::Work {
        st.mode = Mode::Work;
    }
    st.remaining = st.settings.work_minutes as u64 * 60;
    drop(st);
    update_windows(&app);
    emit_state(&app);
    update_tray(&app);
}

#[tauri::command]
fn toggle_pause(app: AppHandle) {
    let state = app.state::<Mutex<AppState>>();
    let paused = {
        let mut st = state.lock().unwrap();
        st.paused = !st.paused;
        st.paused
    };
    let _ = paused;
    emit_state(&app);
    update_tray(&app);
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
    app.exit(0);
}

// ---------------- timer thread ----------------

fn run_timer(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_minute = u64::MAX;
        let mut tip_rng = fastrand::Rng::new();
        let mut previous_tip: Option<String> = None;
        #[cfg(windows)]
        let mut tray_light_theme = None;
        loop {
            std::thread::sleep(Duration::from_secs(1));
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
            let state = app.state::<Mutex<AppState>>();
            let mut st = state.lock().unwrap();
            if st.paused {
                drop(st);
                continue;
            }
            match st.mode {
                Mode::Work => {
                    // 休息刚结束的宽限期内直接走表（起身活动没有键鼠输入是正常的）
                    let idle_limit = idle_reset_limit_secs(&st.settings);
                    let in_grace = st
                        .break_ended_at
                        .map(|t| t.elapsed() < Duration::from_secs(idle_limit))
                        .unwrap_or(false);
                    if !in_grace && idle_secs() >= idle_limit {
                        st.remaining = st.settings.work_minutes as u64 * 60;
                    } else {
                        st.remaining = st.remaining.saturating_sub(1);
                        if st.remaining == 0 {
                            st.mode = Mode::Remind;
                            let break_secs = st.settings.break_minutes as u64 * 60;
                            let postpone_secs = POSTPONE_SECS
                                .min(st.settings.work_minutes as u64 * 60)
                                .max(60);
                            let sound = st.settings.sound;
                            let sat = st.settings.work_minutes;
                            let tip =
                                choose_tip(&st.settings, previous_tip.as_deref(), &mut tip_rng);
                            previous_tip = Some(tip.clone());
                            drop(st);
                            update_windows(&app);
                            let _ = app.emit(
                                "reminder",
                                json!({ "breakSeconds": break_secs, "postponeSeconds": postpone_secs, "sound": sound, "tip": tip, "satMinutes": sat }),
                            );
                            update_tray(&app);
                            last_minute = u64::MAX;
                            continue;
                        }
                    }
                }
                Mode::Break => {
                    st.remaining = st.remaining.saturating_sub(1);
                    if st.remaining == 0 {
                        drop(st);
                        back_to_work(&app);
                        last_minute = u64::MAX;
                        continue;
                    }
                }
                Mode::Remind => {
                    // 用户没理会提醒且已离开电脑 → 视为已经活动过，重置
                    if idle_secs() >= idle_reset_limit_secs(&st.settings) {
                        drop(st);
                        back_to_work(&app);
                        last_minute = u64::MAX;
                        continue;
                    }
                }
            }
            drop(st);

            // macOS 菜单栏每秒显示倒计时；Windows 工作状态仍按分钟更新 tooltip。
            let state = app.state::<Mutex<AppState>>();
            let minute = state.lock().unwrap().remaining / 60;
            let mode_changed = {
                let st = state.lock().unwrap();
                st.mode != Mode::Work
            };
            emit_state(&app);
            if cfg!(target_os = "macos") || mode_changed || minute != last_minute {
                last_minute = minute;
                update_tray(&app);
            }
        }
    });
}

// ---------------- app entry ----------------

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
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 已有实例在运行：把它唤醒（弹出面板），本进程立即退出
            toggle_panel(app, None);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            get_platform,
            get_settings,
            timer_info,
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
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            let mut settings = load_settings_from_disk(&handle);
            let autostart_enabled = autostart_is_enabled(&handle);
            settings.autostart = autostart_enabled;
            handle.manage(Mutex::new(AppState {
                mode: Mode::Work,
                remaining: settings.work_minutes as u64 * 60,
                paused: false,
                break_ended_at: None,
                settings: settings.clone(),
            }));

            // ---- 窗口（在 manage 之后创建，页面加载时状态已就绪）----
            #[cfg(not(target_os = "macos"))]
            let widget =
                WebviewWindowBuilder::new(&handle, "widget", WebviewUrl::App("widget.html".into()))
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
            let reminder = WebviewWindowBuilder::new(
                &handle,
                "reminder",
                WebviewUrl::App("reminder.html".into()),
            )
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
            let settings_win = WebviewWindowBuilder::new(
                &handle,
                "settings",
                WebviewUrl::App("settings.html".into()),
            )
            .title(if zh {
                "Sit Break 设置"
            } else {
                "Sit Break Settings"
            })
            .inner_size(320.0, 580.0)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .accept_first_mouse(true)
            .shadow(false)
            .skip_taskbar(true)
            .resizable(false)
            .center()
            .focused(false)
            .build()?;
            let panel =
                WebviewWindowBuilder::new(&handle, "panel", WebviewUrl::App("panel.html".into()))
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

            handle.manage(PanelState {
                toggled_at: Mutex::new(None),
                auto_hidden_at: Mutex::new(None),
            });

            // ---- 窗口事件：关闭一律隐藏 ----
            reminder.on_window_event({
                let rw = reminder.clone();
                move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = rw.hide();
                        // Alt+F4 关掉提醒视同「再等等」
                        let app = rw.app_handle();
                        postpone(app.clone());
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
                let _ = widget.show();
                position_bottom_right(&widget);
            }
            run_timer(handle.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
            assert!(settings.tips.contains(&tip));
            previous = tip;
        }
    }

    #[test]
    fn custom_tips_beyond_the_first_ten_can_be_selected() {
        let settings = Settings {
            tips: (0..25).map(|i| format!("Custom tip {i}")).collect(),
            ..Settings::default()
        };
        let mut rng = fastrand::Rng::with_seed(7);
        let mut seen = HashSet::new();
        let mut previous: Option<String> = None;
        for _ in 0..1000 {
            let tip = choose_tip(&settings, previous.as_deref(), &mut rng);
            assert!(settings.tips.contains(&tip));
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
        assert!(settings.tips.contains(&tip));
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
    }
}
