//! 指引 frame 的导航策略；用户 HTML 不作为有原生能力的应用页面加载。
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WebviewWindowBuilder};

pub const BOOTSTRAP: &str = include_str!("../guidance-bootstrap.js");

#[derive(Default)]
pub struct GuidanceIsolation {
    online: Mutex<Option<OnlineLoad>>,
    #[cfg(windows)]
    frames: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

struct OnlineLoad {
    session: String,
    initial_url: tauri::Url,
    current_url: tauri::Url,
    initial_pending: bool,
    sealed: bool,
    prepared: std::time::Instant,
    #[cfg(windows)]
    navigation_id: Option<u64>,
}

impl GuidanceIsolation {
    /// Windows 必须等此 frame 的原生导航钩子真正安装后才返回用户页面。
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

    /// 宿主只为当前指引创建的一次初始加载授权，不能当作通用网页导航白名单。
    pub fn prepare_online(&self, session: &str, url: &str) -> Result<(), String> {
        self.wait_until_ready(session)?;
        if !valid_online_document(url) {
            return Err("html-isolation-unavailable".into());
        }
        let url = tauri::Url::parse(url).map_err(|_| "html-isolation-unavailable")?;
        *self.online.lock().unwrap() = Some(OnlineLoad {
            session: session.to_owned(),
            initial_url: url.clone(),
            current_url: url,
            initial_pending: true,
            sealed: false,
            prepared: std::time::Instant::now(),
            #[cfg(windows)]
            navigation_id: None,
        });
        Ok(())
    }

    pub fn confirm_online(&self, session: &str, document_url: &str) -> Result<(), String> {
        if !valid_online_document(document_url) {
            return Err("html-isolation-unavailable".into());
        }
        let url = tauri::Url::parse(document_url).map_err(|_| "html-isolation-unavailable")?;
        let mut online = self.online.lock().unwrap();
        let load = online.as_mut().ok_or("html-isolation-unavailable")?;
        if load.session != session || load.initial_pending {
            return Err("html-isolation-unavailable".into());
        }
        #[cfg(windows)]
        if without_fragment(&load.current_url) != without_fragment(&url) {
            return Err("html-isolation-unavailable".into());
        }
        // WKWebView 的服务器重定向不一定再次触发 action callback；只有已消费的初始导航可确认。
        load.current_url = url;
        load.sealed = true;
        Ok(())
    }

    pub fn release(&self, session: &str) {
        let mut online = self.online.lock().unwrap();
        if online.as_ref().is_some_and(|load| load.session == session) {
            *online = None;
        }
    }

    #[cfg(not(windows))]
    fn allow_initial(&self, url: &tauri::Url) -> bool {
        let mut online = self.online.lock().unwrap();
        let Some(load) = online.as_mut() else {
            return false;
        };
        if !load.initial_pending
            && url.fragment().is_some()
            && without_fragment(&load.current_url) == without_fragment(url)
        {
            return true;
        }
        if load.initial_pending
            && !load.sealed
            && load.prepared.elapsed() < std::time::Duration::from_secs(15)
            && without_fragment(&load.initial_url) == without_fragment(url)
        {
            load.initial_pending = false;
            return true;
        }
        false
    }

    #[cfg(windows)]
    fn allow_online_navigation(
        &self,
        session: &str,
        url: &tauri::Url,
        navigation_id: u64,
        redirected: bool,
    ) -> bool {
        if !valid_online_document(url.as_str()) {
            return false;
        }
        let mut online = self.online.lock().unwrap();
        let Some(load) = online.as_mut() else {
            return false;
        };
        if load.session == session
            && !load.initial_pending
            && url.fragment().is_some()
            && without_fragment(&load.current_url) == without_fragment(url)
        {
            return true;
        }
        if load.session != session
            || load.sealed
            || load.prepared.elapsed() >= std::time::Duration::from_secs(15)
        {
            return false;
        }
        let initial = load.initial_pending
            && !redirected
            && without_fragment(&load.initial_url) == without_fragment(url);
        let redirect =
            !load.initial_pending && redirected && load.navigation_id == Some(navigation_id);
        if !initial && !redirect {
            return false;
        }
        load.initial_pending = false;
        load.navigation_id = Some(navigation_id);
        load.current_url = url.clone();
        true
    }

    #[cfg(windows)]
    fn navigation_completed(&self, session: &str, navigation_id: u64) {
        let mut online = self.online.lock().unwrap();
        if let Some(load) = online.as_mut() {
            if load.session == session && load.navigation_id == Some(navigation_id) {
                load.sealed = true;
            }
        }
    }
}

fn without_fragment(url: &tauri::Url) -> &str {
    url.as_str().split('#').next().unwrap_or_default()
}

pub fn valid_online_document(url: &str) -> bool {
    crate::valid_guidance_url(url)
        && tauri::Url::parse(url).is_ok_and(|url| {
            // Tauri 的 Windows app/custom-protocol origin 是 *.localhost，不能把它当远程网页授权。
            !url.host_str().is_some_and(|host| {
                host.trim_end_matches('.')
                    .to_ascii_lowercase()
                    .ends_with(".localhost")
            })
        })
}

fn open_external<R: tauri::Runtime>(app: &AppHandle<R>, url: &str) {
    if !crate::valid_guidance_url(url) {
        return;
    }
    let app = app.clone();
    let url = url.to_owned();
    tauri::async_runtime::spawn(async move {
        if crate::open_guidance_url(url).await.is_err() {
            let _ = app.emit("guidance-browser-error", ());
        }
    });
}

fn allow_frame_navigation<R: tauri::Runtime>(app: &AppHandle<R>, url: &tauri::Url) -> bool {
    if url.scheme() == "about" && matches!(url.path(), "blank" | "srcdoc") {
        return true;
    }
    #[cfg(not(windows))]
    if valid_online_document(url.as_str()) && app.state::<GuidanceIsolation>().allow_initial(url) {
        return true;
    }
    open_external(app, url.as_str());
    false
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
    let new_window_app = app.clone();
    let initial_document = std::sync::atomic::AtomicBool::new(false);
    builder
        .initialization_script_for_all_frames(BOOTSTRAP)
        .on_navigation(move |url| {
            // macOS 回调同时收到子帧；只允许应用初始文档，之后不让指引导航到有原生能力的页面。
            if is_application_document(&navigation_app, url) {
                return !initial_document.swap(true, std::sync::atomic::Ordering::Relaxed);
            }
            allow_frame_navigation(&navigation_app, url)
        })
        .on_new_window(move |url, _| {
            open_external(&new_window_app, url.as_str());
            tauri::webview::NewWindowResponse::Deny
        })
}

#[cfg(not(windows))]
pub fn install_frame_hooks(_window: &WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn install_frame_hooks(window: &WebviewWindow) -> tauri::Result<()> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2Frame2, ICoreWebView2_4};
    use webview2_com::{
        FrameCreatedEventHandler, FrameDestroyedEventHandler, FrameNavigationCompletedEventHandler,
        FrameNavigationStartingEventHandler,
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
                    let navigation_app = app.clone();
                    let navigation_session = session.clone();
                    let mut navigation_token = 0;
                    guarded.add_NavigationStarting(
                        &FrameNavigationStartingEventHandler::create(Box::new(move |_, event| {
                            let Some(event) = event else {
                                return Ok(());
                            };
                            let mut uri = PWSTR::null();
                            event.Uri(&mut uri)?;
                            let uri = webview2_com::take_pwstr(uri);
                            let mut redirected = false.into();
                            let mut navigation_id = 0;
                            event.IsRedirected(&mut redirected)?;
                            event.NavigationId(&mut navigation_id)?;
                            let allow = tauri::Url::parse(&uri).is_ok_and(|url| {
                                if navigation_app
                                    .state::<GuidanceIsolation>()
                                    .allow_online_navigation(
                                        &navigation_session,
                                        &url,
                                        navigation_id,
                                        redirected.as_bool(),
                                    )
                                {
                                    true
                                } else {
                                    allow_frame_navigation(&navigation_app, &url)
                                }
                            });
                            event.SetCancel(!allow)?;
                            Ok(())
                        })),
                        &mut navigation_token,
                    )?;
                    let completed_app = app.clone();
                    let completed_session = session.clone();
                    let mut completed_token = 0;
                    guarded.add_NavigationCompleted(
                        &FrameNavigationCompletedEventHandler::create(Box::new(move |_, event| {
                            if let Some(event) = event {
                                let mut navigation_id = 0;
                                event.NavigationId(&mut navigation_id)?;
                                completed_app
                                    .state::<GuidanceIsolation>()
                                    .navigation_completed(&completed_session, navigation_id);
                            }
                            Ok(())
                        })),
                        &mut completed_token,
                    )?;
                    let destroyed_frames = frames.clone();
                    let destroyed_session = session.clone();
                    let destroyed_app = app.clone();
                    let mut destroyed_token = 0;
                    frame.add_Destroyed(
                        &FrameDestroyedEventHandler::create(Box::new(move |_, _| {
                            destroyed_frames.lock().unwrap().remove(&destroyed_session);
                            destroyed_app
                                .state::<GuidanceIsolation>()
                                .release(&destroyed_session);
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
