use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};

const IDLE_RESET_SECS: u64 = 120;
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
            show_widget: true,
            autostart: false,
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
    app.path().app_data_dir().ok().map(|d| d.join("settings.json"))
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

#[cfg(windows)]
fn idle_secs() -> u64 {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            (GetTickCount().wrapping_sub(info.dwTime)) as u64 / 1000
        } else {
            0
        }
    }
}

#[cfg(not(windows))]
fn idle_secs() -> u64 {
    0
}

fn position_bottom_right(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let ms = monitor.size();
    let ws = window.outer_size().unwrap_or_default();
    let margin = (24.0 * monitor.scale_factor()) as i32;
    let x = ms.width as i32 - ws.width as i32 - margin;
    let y = ms.height as i32 - ws.height as i32 - margin;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn position_above_taskbar(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let ms = monitor.size();
    let ws = window.outer_size().unwrap_or_default();
    let margin = (16.0 * monitor.scale_factor()) as i32;
    let taskbar = (60.0 * monitor.scale_factor()) as i32;
    let x = ms.width as i32 - ws.width as i32 - margin;
    let y = ms.height as i32 - ws.height as i32 - taskbar;
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
    drop(panel_state);
    if let Some(panel) = app.get_webview_window("panel") {
        if panel.is_visible().unwrap_or(false) {
            let _ = panel.hide();
        } else if recent_auto_hide && cursor.is_some() {
            // 托盘点击：刚被本次点击收起，保持隐藏
        } else {
            match cursor {
                Some(c) => position_near_tray(&panel, c),
                None => position_above_taskbar(&panel),
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
    let mut x = cursor.x - ws.width as f64 / 2.0;
    let mut y = cursor.y - ws.height as f64 - 8.0;
    if let Some(m) = &monitor {
        let (mx, my) = (m.position().x as f64, m.position().y as f64);
        let (mw, mh) = (m.size().width as f64, m.size().height as f64);
        if x < mx {
            x = mx + 8.0;
        }
        if x + ws.width as f64 > mx + mw {
            x = mx + mw - ws.width as f64 - 8.0;
        }
        if y < my {
            // 上方放不下（如任务栏在顶部）→ 放到光标下方
            y = cursor.y + 8.0;
        }
        if y + ws.height as f64 > my + mh {
            y = my + mh - ws.height as f64 - 8.0;
        }
    }
    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
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

fn update_tray(app: &AppHandle) {
    let st = app.state::<Mutex<AppState>>();
    let st = st.lock().unwrap();
    let tooltip = match st.mode {
        Mode::Work if st.paused => {
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
    emit_state(&app);
    update_tray(&app);
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
    persist(&app, &app.state::<Mutex<AppState>>().lock().unwrap().settings);
    emit_state(&app);
    update_tray(&app);
}

#[tauri::command]
fn apply_break(app: AppHandle, minutes: u32) {
    let state = app.state::<Mutex<AppState>>();
    let mut st = state.lock().unwrap();
    st.settings.break_minutes = minutes;
    drop(st);
    persist(&app, &app.state::<Mutex<AppState>>().lock().unwrap().settings);
}

// ---------------- Tauri commands ----------------

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    app.state::<Mutex<AppState>>().lock().unwrap().settings.clone()
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
fn save_settings(
    app: AppHandle,
    settings: Settings,
) -> Result<(), String> {
    let al = app.autolaunch();
    let enabled = al.is_enabled().unwrap_or(false);
    if settings.autostart != enabled {
        if settings.autostart {
            al.enable().map_err(|e| e.to_string())?;
        } else {
            al.disable().map_err(|e| e.to_string())?;
        }
    }
    let widget_size = settings.widget_size.clamp(40, 140);
    let mut settings = Settings { widget_size, ..settings };

    // 切换语言时，若提示列表仍是另一语言的默认文案，则跟随切换（不覆盖自定义内容）
    let zh_now = is_zh(&settings);
    let other_defaults = if zh_now { default_tips_en() } else { default_tips() };
    if settings.tips == other_defaults {
        settings.tips = if zh_now { default_tips() } else { default_tips_en() };
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
    drop(state);
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
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let state = app.state::<Mutex<AppState>>();
            let mut st = state.lock().unwrap();
            if st.paused {
                drop(st);
                continue;
            }
            match st.mode {
                Mode::Work => {
                    // 休息刚结束的宽限期内直接走表（起身活动没有键鼠输入是正常的）
                    let in_grace = st
                        .break_ended_at
                        .map(|t| t.elapsed() < Duration::from_secs(IDLE_RESET_SECS))
                        .unwrap_or(false);
                    if !in_grace && idle_secs() >= IDLE_RESET_SECS {
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
                            let tip_idx = (std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_nanos() as usize)
                                .unwrap_or(0))
                                % TIPS.len();
                            let tip = {
                                let list = &st.settings.tips;
                                let n = if list.is_empty() { TIPS.len() } else { list.len() };
                                let idx = tip_idx % n;
                                if list.is_empty() {
                                    TIPS[idx].to_string()
                                } else {
                                    list[idx].clone()
                                }
                            };
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
                    if idle_secs() >= IDLE_RESET_SECS {
                        drop(st);
                        back_to_work(&app);
                        last_minute = u64::MAX;
                        continue;
                    }
                }
            }
            drop(st);

            // 悬浮条每秒走字；托盘 tooltip 每分钟刷新一次
            let state = app.state::<Mutex<AppState>>();
            let minute = state.lock().unwrap().remaining / 60;
            let mode_changed = {
                let st = state.lock().unwrap();
                st.mode != Mode::Work
            };
            drop(state);
            emit_state(&app);
            if mode_changed || minute != last_minute {
                last_minute = minute;
                update_tray(&app);
            }
        }
    });
}

// ---------------- app entry ----------------

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
            let handle = app.handle().clone();
            let mut settings = load_settings_from_disk(&handle);
            let autostart_enabled = handle.autolaunch().is_enabled().unwrap_or(false);
            settings.autostart = autostart_enabled;
            handle.manage(Mutex::new(AppState {
                mode: Mode::Work,
                remaining: settings.work_minutes as u64 * 60,
                paused: false,
                break_ended_at: None,
                settings: settings.clone(),
            }));

            // ---- 窗口（在 manage 之后创建，页面加载时状态已就绪）----
            let widget = WebviewWindowBuilder::new(
                &handle,
                "widget",
                WebviewUrl::App("widget.html".into()),
            )
            .title("Sit Break")
            .inner_size(
                (settings.widget_size as f64 + 24.0).max(140.0),
                (settings.widget_size as f64 + 24.0).max(140.0),
            )
            .visible(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
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
            .title(if zh { "Sit Break 提醒" } else { "Sit Break Reminder" })
            .inner_size(380.0, 368.0)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .build()?;
            let settings_win = WebviewWindowBuilder::new(
                &handle,
                "settings",
                WebviewUrl::App("settings.html".into()),
            )
            .title(if zh { "Sit Break 设置" } else { "Sit Break Settings" })
            .inner_size(320.0, 580.0)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .skip_taskbar(true)
            .resizable(false)
            .center()
            .build()?;
            let panel = WebviewWindowBuilder::new(
                &handle,
                "panel",
                WebviewUrl::App("panel.html".into()),
            )
            .title("Sit Break")
            .inner_size(272.0, 340.0)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .build()?;

            // ---- 托盘（左/右键均弹出面板，无原生菜单）----
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
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
                            *app.state::<PanelState>()
                                .auto_hidden_at
                                .lock()
                                .unwrap() = Some(Instant::now());
                        }
                    }
                    _ => {}
                }
            });

            update_tray(&handle);
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
