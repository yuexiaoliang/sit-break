//! 单文件 HTML 的原生导航边界；仅允许隔离的 srcdoc 与空白页。
use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder};

pub const BOOTSTRAP: &str = include_str!("../guidance-bootstrap.js");

#[derive(Default)]
pub struct GuidanceIsolation {
    #[cfg(windows)]
    frames: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

impl GuidanceIsolation {
    pub fn wait_until_ready(&self, session: &str) -> Result<(), String> {
        if !session
            .strip_prefix("sit-break-guidance-")
            .is_some_and(|id| id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err("html-isolation-unavailable".into());
        }
        #[cfg(windows)]
        {
            let started = std::time::Instant::now();
            while started.elapsed() < std::time::Duration::from_secs(3) {
                if self.frames.lock().unwrap().contains(session) {
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err("html-isolation-unavailable".into())
        }
        #[cfg(not(windows))]
        Ok(())
    }
}

fn allow_frame_navigation(url: &tauri::Url) -> bool {
    url.scheme() == "about" && matches!(url.path(), "blank" | "srcdoc")
}

fn is_application_document<R: tauri::Runtime>(app: &AppHandle<R>, url: &tauri::Url) -> bool {
    let application_origin = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"));
    let development_origin = cfg!(debug_assertions)
        && app
            .config()
            .build
            .dev_url
            .as_ref()
            .is_some_and(|dev| dev.origin() == url.origin());
    (application_origin || development_origin) && url.path() == "/reminder.html"
}

pub fn builder<'a, R: tauri::Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    app: &AppHandle<R>,
) -> WebviewWindowBuilder<'a, R, M> {
    let navigation_app = app.clone();
    let initial_document = std::sync::atomic::AtomicBool::new(false);
    builder
        .initialization_script_for_all_frames(BOOTSTRAP)
        .on_navigation(move |url| {
            // macOS 回调同时收到子帧；只允许应用初始文档，之后不让指引导航到有原生能力的页面。
            if is_application_document(&navigation_app, url) {
                return !initial_document.swap(true, std::sync::atomic::Ordering::Relaxed);
            }
            allow_frame_navigation(url)
        })
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
}

#[cfg(not(windows))]
pub fn install_frame_hooks(_window: &WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn install_frame_hooks(window: &WebviewWindow) -> tauri::Result<()> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2Frame2, ICoreWebView2_4};
    use webview2_com::{
        FrameCreatedEventHandler, FrameDestroyedEventHandler, FrameNavigationStartingEventHandler,
    };
    use windows_core_61::{Interface, PWSTR};
    let app = window.app_handle().clone();
    let frames = app.state::<GuidanceIsolation>().frames.clone();
    window.with_webview(move |webview| unsafe {
        let result = (|| -> windows_core_61::Result<()> {
            let core: ICoreWebView2_4 = webview.controller().CoreWebView2()?.cast()?;
            let mut token = 0;
            core.add_FrameCreated(
                &FrameCreatedEventHandler::create(Box::new(move |_, event| {
                    let Some(event) = event else {
                        return Ok(());
                    };
                    let frame = event.Frame()?;
                    let mut name = PWSTR::null();
                    frame.Name(&mut name)?;
                    let session = webview2_com::take_pwstr(name);
                    if !session.starts_with("sit-break-guidance-") {
                        return Ok(());
                    }
                    let guarded: ICoreWebView2Frame2 = frame.cast()?;
                    let mut navigation_token = 0;
                    guarded.add_NavigationStarting(
                        &FrameNavigationStartingEventHandler::create(Box::new(move |_, event| {
                            let Some(event) = event else {
                                return Ok(());
                            };
                            let mut uri = PWSTR::null();
                            event.Uri(&mut uri)?;
                            let uri = webview2_com::take_pwstr(uri);
                            let allow = tauri::Url::parse(&uri)
                                .is_ok_and(|url| allow_frame_navigation(&url));
                            event.SetCancel(!allow)?;
                            Ok(())
                        })),
                        &mut navigation_token,
                    )?;
                    let destroyed_frames = frames.clone();
                    let destroyed_session = session.clone();
                    let mut destroyed_token = 0;
                    frame.add_Destroyed(
                        &FrameDestroyedEventHandler::create(Box::new(move |_, _| {
                            destroyed_frames.lock().unwrap().remove(&destroyed_session);
                            Ok(())
                        })),
                        &mut destroyed_token,
                    )?;
                    frames.lock().unwrap().insert(session);
                    Ok(())
                })),
                &mut token,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("无法安装动作指引隔离导航：{error}");
        }
    })
}
