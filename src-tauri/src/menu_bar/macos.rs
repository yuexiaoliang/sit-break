use std::sync::Mutex;

use block2::RcBlock;
use objc2::{
    rc::Retained,
    runtime::{AnyObject, Bool},
    AnyThread, MainThreadMarker,
};
use objc2_app_kit::{
    NSAccessibility, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSAttributedStringNSStringDrawing, NSAutoresizingMaskOptions, NSBezierPath,
    NSCellImagePosition, NSColor, NSCompositingOperation, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSGraphicsContext, NSImage, NSImageScaling, NSImageView,
    NSRectFillUsingOperation, NSStatusBarButton, NSUserInterfaceItemIdentification, NSView,
    NSWindowOrderingMode,
};
use objc2_core_graphics::CGContext;
use objc2_foundation::{
    NSArray, NSAttributedString, NSData, NSDictionary, NSPoint, NSRect, NSSize, NSString,
};
use tauri::{AppHandle, Manager};

use crate::{is_zh, timer::TimerApp};

use super::{Fill, Icon, Status};

const CONTENT_HEIGHT: f64 = 20.0;
const ICON_SIZE: f64 = 16.0;
const PADDING: f64 = 6.0;
const TEXT_X: f64 = PADDING + ICON_SIZE + 4.0;
const BACKGROUND_OPACITY: f64 = 0.75;
const BACKGROUND_ID: &str = "SitBreakMenuBarProgress";

pub(crate) fn update(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(tray) = handle.tray_by_id("main-tray") else {
            return;
        };
        let (status, description) = {
            let timer = handle.state::<Mutex<TimerApp>>();
            let timer = timer.lock().unwrap();
            let zh = is_zh(timer.settings());
            let status = Status::from_timer(&timer, zh);
            let description = status.description(zh);
            (status, description)
        };
        let accessibility_label = description.clone();
        let _ = tray.with_inner_tray_icon(move |native| {
            let Some((item, main_thread)) = native.ns_status_item().zip(MainThreadMarker::new())
            else {
                return;
            };
            if let Some(button) = item.button(main_thread) {
                let dark = uses_dark_appearance(&button);
                button.setImagePosition(NSCellImagePosition::ImageOnly);
                button.setImage(Some(&status_image(&button, &status, dark)));
                update_background(&button, main_thread, dark, status.progress);
                button.setAccessibilityLabel(Some(&NSString::from_str(&accessibility_label)));
            }
        });
        // Tauri 会用当前按钮宽度更新鼠标事件区域，确保整条背景都能点开面板。
        let _ = tray.set_tooltip(Some(description));
    });
}

pub(crate) fn set_pressed(tray: &tauri::tray::TrayIcon, pressed: bool) {
    let _ = tray.with_inner_tray_icon(move |native| {
        let button =
            MainThreadMarker::new().and_then(|marker| native.ns_status_item()?.button(marker));
        let Some(button) = button else { return };
        let Some(view) = button
            .window()
            .and_then(|window| window.contentView())
            .and_then(|content| background_view(&content))
        else {
            return;
        };
        // Tauri 投递点击事件后会触发系统矩形高亮。用同一圆角背景呈现按下反馈。
        button.highlight(false);
        view.setAlphaValue(if pressed { 0.84 } else { 1.0 });
    });
}

fn uses_dark_appearance(button: &NSStatusBarButton) -> bool {
    // SAFETY: AppKit defines both appearance constants as immutable strings.
    let (aqua, dark_aqua) = unsafe { (NSAppearanceNameAqua, NSAppearanceNameDarkAqua) };
    button
        .effectiveAppearance()
        .bestMatchFromAppearancesWithNames(&NSArray::from_slice(&[aqua, dark_aqua]))
        .is_some_and(|name| &*name == dark_aqua)
}

fn status_image(button: &NSStatusBarButton, status: &Status, dark: bool) -> Retained<NSImage> {
    let foreground = srgb(if dark { 0xf5f5f7 } else { 0x1c1c1e }, 1.0);
    let font = button
        .font()
        .unwrap_or_else(|| NSFont::menuBarFontOfSize(0.0));
    // SAFETY: These two AppKit keys require NSFont and NSColor values, respectively.
    let text = unsafe {
        let attributes = NSDictionary::from_slices(
            &[NSFontAttributeName, NSForegroundColorAttributeName],
            &[&*font as &AnyObject, &*foreground as &AnyObject],
        );
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(&status.label),
            Some(&attributes),
        )
    };
    let text_size = text.size();
    let size = NSSize::new(TEXT_X + text_size.width.ceil() + PADDING, CONTENT_HEIGHT);
    let icon = load_icon(status.icon);
    let draw = RcBlock::new(move |bounds: NSRect| {
        if let Some(icon) = &icon {
            draw_icon(icon, &foreground, bounds);
        }
        text.drawAtPoint(NSPoint::new(
            bounds.origin.x + TEXT_X,
            bounds.origin.y + (bounds.size.height - text_size.height) / 2.0,
        ));
        Bool::YES
    });
    NSImage::imageWithSize_flipped_drawingHandler(size, false, &draw)
}

fn update_background(
    button: &NSStatusBarButton,
    main_thread: MainThreadMarker,
    dark: bool,
    progress: Option<(Fill, f64)>,
) {
    let Some(content) = button.window().and_then(|window| window.contentView()) else {
        return;
    };
    let bounds = content.bounds();
    if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
        return;
    }

    // 用系统条目窗口的内容区域绘制背景。按钮图像的内边距只用于图标和文字。
    let view = background_view(&content).unwrap_or_else(|| {
        let view = NSImageView::initWithFrame(main_thread.alloc(), bounds);
        view.setIdentifier(Some(&NSString::from_str(BACKGROUND_ID)));
        view.setImageScaling(NSImageScaling::ScaleAxesIndependently);
        view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        view.setAccessibilityElement(false);
        content.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, None);
        view
    });
    view.setFrame(bounds);
    view.setImage(Some(&background_image(bounds.size, dark, progress)));
}

fn background_view(content: &NSView) -> Option<Retained<NSImageView>> {
    let identifier = NSString::from_str(BACKGROUND_ID);
    content.subviews().iter().find_map(|view| {
        view.downcast::<NSImageView>()
            .ok()
            .filter(|view| view.identifier().as_deref() == Some(&*identifier))
    })
}

fn background_image(size: NSSize, dark: bool, progress: Option<(Fill, f64)>) -> Retained<NSImage> {
    let track = srgb(if dark { 0x323234 } else { 0xe1e3e6 }, BACKGROUND_OPACITY);
    let fill = progress.map(|(fill, remaining)| {
        let color = match (dark, fill) {
            (true, Fill::Work) => 0x3d596c,
            (true, Fill::Reminder) => 0x69553b,
            (true, Fill::Break) => 0x3a6053,
            (true, Fill::Paused) => 0x57585c,
            (false, Fill::Work) => 0xa8c1d4,
            (false, Fill::Reminder) => 0xe3c596,
            (false, Fill::Break) => 0xb2d3c3,
            (false, Fill::Paused) => 0xb5b9c0,
        };
        (srgb(color, BACKGROUND_OPACITY), remaining)
    });

    let draw = RcBlock::new(move |bounds: NSRect| {
        let radius = bounds.size.width.min(bounds.size.height) / 3.0;
        let shape = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, radius, radius);
        NSGraphicsContext::saveGraphicsState_class();
        shape.addClip();
        track.setFill();
        NSRectFillUsingOperation(bounds, NSCompositingOperation::Copy);
        if let Some((color, remaining)) = &fill {
            color.setFill();
            NSRectFillUsingOperation(
                NSRect::new(
                    bounds.origin,
                    NSSize::new(bounds.size.width * remaining, bounds.size.height),
                ),
                NSCompositingOperation::Copy,
            );
        }
        NSGraphicsContext::restoreGraphicsState_class();
        Bool::YES
    });
    NSImage::imageWithSize_flipped_drawingHandler(size, false, &draw)
}

fn load_icon(icon: Icon) -> Option<Retained<NSImage>> {
    let symbol = match icon {
        Icon::Ring => None,
        Icon::Paused => Some("pause.fill"),
        Icon::Muted => Some("bell.slash"),
    };
    symbol
        .and_then(|name| {
            NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &NSString::from_str(name),
                None,
            )
        })
        .or_else(|| {
            NSImage::initWithData(
                NSImage::alloc(),
                &NSData::with_bytes(include_bytes!("../../icons/tray-icon.png")),
            )
        })
}

fn draw_icon(icon: &NSImage, color: &NSColor, bounds: NSRect) {
    let Some(context) = NSGraphicsContext::currentContext() else {
        return;
    };
    let size = icon.size();
    let scale = ICON_SIZE / size.width.max(size.height);
    let width = size.width * scale;
    let height = size.height * scale;
    let rect = NSRect::new(
        NSPoint::new(
            bounds.origin.x + PADDING + (ICON_SIZE - width) / 2.0,
            bounds.origin.y + (bounds.size.height - height) / 2.0,
        ),
        NSSize::new(width, height),
    );

    // SAFETY: Draw into an isolated transparency layer with no auxiliary options.
    let context = context.CGContext();
    unsafe { CGContext::begin_transparency_layer_with_rect(Some(&context), rect, None) };
    icon.drawInRect(rect);
    color.setFill();
    NSRectFillUsingOperation(rect, NSCompositingOperation::SourceIn);
    CGContext::end_transparency_layer(Some(&context));
}

fn srgb(rgb: u32, alpha: f64) -> Retained<NSColor> {
    let component = |shift| f64::from((rgb >> shift) & 0xff_u32) / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(component(16), component(8), component(0), alpha)
}
