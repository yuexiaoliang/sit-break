//! 使用生产定位代码和真实原生窗口，检查尺寸切换后是否完整落在工作区内。
//! cargo run --example reminder_window_check -- <报告路径>

#[path = "../src/window_geometry.rs"]
#[allow(dead_code)] // 与产品共用模块；本检查不调用独立悬浮球的定位入口。
mod window_geometry;

use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{WebviewUrl, WebviewWindowBuilder};

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("需要报告路径");
    let mut context = tauri::generate_context!();
    context.config_mut().identifier = "com.sitbreak.windowcheck".into();
    let outcome = Arc::new(AtomicBool::new(false));
    let result = outcome.clone();
    let app = tauri::Builder::default()
        .setup(move |app| {
            let window = WebviewWindowBuilder::new(
                app,
                "reminder",
                WebviewUrl::External("about:blank".parse()?),
            )
            .title("Sit Break 定位检查")
            .inner_size(380.0, 368.0)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .resizable(false)
            .focused(false)
            .build()?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut samples = Vec::new();
                let mut passed = true;
                for (name, media, main_thread) in [
                    ("主线程首次视频休息", true, true),
                    ("主线程返回纯文字", false, true),
                    ("主线程纯文字切换视频", true, true),
                    ("主线程重复视频休息", true, true),
                    ("后台线程返回纯文字", false, false),
                    ("后台线程切换视频", true, false),
                ] {
                    // 同步 IPC 在主线程执行；计时线程和恢复入口也需覆盖。
                    if main_thread {
                        let current = window.clone();
                        window
                            .run_on_main_thread(move || {
                                window_geometry::show_reminder(&current, media).unwrap()
                            })
                            .unwrap();
                    } else {
                        window_geometry::show_reminder(&window, media).unwrap();
                    }
                    std::thread::sleep(Duration::from_millis(300));
                    let position = window.outer_position().unwrap();
                    let size = window.outer_size().unwrap();
                    let monitor = window.current_monitor().unwrap().unwrap();
                    let area = monitor.work_area();
                    let inside = position.x >= area.position.x
                        && position.y >= area.position.y
                        && i64::from(position.x) + i64::from(size.width)
                            <= i64::from(area.position.x) + i64::from(area.size.width)
                        && i64::from(position.y) + i64::from(size.height)
                            <= i64::from(area.position.y) + i64::from(area.size.height);
                    passed &= inside;
                    samples.push(json!({
                        "name": name, "inside": inside, "scale": monitor.scale_factor(),
                        "window": {"position": position, "size": size},
                        "workArea": {"position": area.position, "size": area.size}
                    }));
                }
                let report = json!({"passed": passed, "samples": samples});
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
                std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
                result.store(passed, Ordering::SeqCst);
                handle.exit(if passed { 0 } else { 1 });
            });
            Ok(())
        })
        .build(context)
        .expect("无法创建原生检查应用");
    app.run_return(|_, _| {});
    std::process::exit(if outcome.load(Ordering::SeqCst) { 0 } else { 1 });
}
