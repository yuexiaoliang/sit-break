use super::{finish_panel_setup, panel_builder, request_panel_open};
use std::sync::{Arc, Mutex};
use tauri::{Manager, WebviewWindowBuilder};

#[test]
fn opening_during_plugin_initialization_waits_for_the_panel_and_later_opens_remain_available() {
    let opening_results = Arc::new(Mutex::new(Vec::new()));
    let during_initialization = opening_results.clone();
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().identifier = "com.sitbreak.startup-test".into();
    // 只替代原生运行时；测试插件在与单实例插件相同的初始化边界提交打开请求。
    let app = panel_builder(tauri::test::mock_builder())
        .plugin(
            tauri::plugin::Builder::<_, ()>::new("startup-opening-test")
                .setup(move |app, _| {
                    assert!(app.get_webview_window("panel").is_none());
                    for _ in 0..2 {
                        during_initialization
                            .lock()
                            .unwrap()
                            .push(request_panel_open(app, None)?);
                    }
                    Ok(())
                })
                .build(),
        )
        .build(context)
        .unwrap();
    assert_eq!(*opening_results.lock().unwrap(), [false, false]);

    WebviewWindowBuilder::new(&app, "panel", Default::default())
        .visible(false)
        .build()
        .unwrap();
    assert!(finish_panel_setup(app.handle()).unwrap());
    assert!(!finish_panel_setup(app.handle()).unwrap());
    assert!(request_panel_open(app.handle(), None).unwrap());
    assert!(request_panel_open(app.handle(), None).unwrap());
}
