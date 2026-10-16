use tauri::{Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

#[cfg(not(target_os = "macos"))]
pub(crate) fn position_bottom_right(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let size = window.outer_size().unwrap_or_default();
    let _ = window.set_position(bottom_right_position(&monitor, size));
}

fn bottom_right_position(monitor: &Monitor, size: PhysicalSize<u32>) -> PhysicalPosition<i32> {
    let area = monitor.work_area();
    let margin = (24.0 * monitor.scale_factor()) as i32;
    let x = clamp_axis(
        i32::MAX,
        area.position.x,
        area.size.width,
        size.width,
        margin,
    );
    let y = clamp_axis(
        i32::MAX,
        area.position.y,
        area.size.height,
        size.height,
        margin,
    );
    PhysicalPosition::new(x, y)
}

pub(crate) fn clamp_axis(
    candidate: i32,
    origin: i32,
    available: u32,
    window: u32,
    margin: i32,
) -> i32 {
    if window >= available {
        return origin;
    }
    let margin = margin.min((available - window) as i32 / 2).max(0);
    candidate.clamp(
        origin + margin,
        origin + available as i32 - window as i32 - margin,
    )
}

pub(crate) fn show_reminder(r: &WebviewWindow, media: bool) -> tauri::Result<()> {
    layout_reminder(r, media, 368.0)?;
    r.show()
}

pub(crate) fn fit_text_break(r: &WebviewWindow, height: u32) -> tauri::Result<()> {
    layout_reminder(r, false, f64::from(height.clamp(244, 368)))
}

fn layout_reminder(r: &WebviewWindow, media: bool, text_height: f64) -> tauri::Result<()> {
    let (mut width, mut height): (f64, f64) = if media {
        (624.0, 344.0)
    } else {
        (380.0, text_height)
    };
    if let Some(monitor) = r
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| r.primary_monitor().ok().flatten())
    {
        let area = monitor.work_area();
        let available_width = (f64::from(area.size.width) / monitor.scale_factor() - 24.0).max(1.0);
        if media && available_width < 560.0 {
            height = 460.0;
        }
        width = width.min(available_width);
        height = height.min((f64::from(area.size.height) / monitor.scale_factor() - 24.0).max(1.0));
        // macOS 在主线程异步调整窗口尺寸；紧接着读取 outer_size 仍可能是旧值。
        // 大小和位置使用同一显示器与目标物理尺寸，避免视频变宽后右侧越界。
        let size = tauri::LogicalSize::new(width, height).to_physical(monitor.scale_factor());
        r.set_size(size)?;
        r.set_position(bottom_right_position(&monitor, size))?;
    } else {
        r.set_size(tauri::LogicalSize::new(width, height))?;
    }
    Ok(())
}
