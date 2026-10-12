#[cfg(windows)]
pub fn idle_secs() -> u64 {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    // SAFETY: LASTINPUTINFO has the required size and remains valid during the call.
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            GetTickCount().wrapping_sub(info.dwTime) as u64 / 1000
        } else {
            0
        }
    }
}

#[cfg(windows)]
pub fn system_uses_light_theme() -> Option<bool> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let personalize = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .ok()?;
    personalize
        .get_value::<u32, _>("SystemUsesLightTheme")
        .ok()
        .map(|value| value != 0)
}

#[cfg(target_os = "macos")]
mod macos {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
    }

    pub fn idle_secs() -> u64 {
        // CombinedSessionState (0) covers the current login session; AnyInputEvent
        // (UINT32_MAX) includes keyboard, mouse and tablet input.
        // SAFETY: This public CoreGraphics query takes scalar values, retains no
        // pointers and does not install an event tap or monitor individual events.
        let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
        if seconds.is_finite() && seconds >= 0.0 {
            seconds as u64
        } else {
            0
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn core_graphics_idle_query_is_available() {
            let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
            assert!(
                seconds.is_finite() && seconds >= 0.0,
                "invalid idle time: {seconds}"
            );
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::idle_secs;

#[cfg(target_os = "macos")]
pub fn configure_menu_bar(tray: &tauri::tray::TrayIcon) -> tauri::Result<()> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSFont, NSFontWeightRegular};

    // Tauri runs this closure on the main thread. Monospaced digits prevent the
    // status item and adjacent menu-bar icons from shifting with every second.
    tray.with_inner_tray_icon(|native| {
        if let (Some(item), Some(mtm)) = (native.ns_status_item(), MainThreadMarker::new()) {
            if let Some(button) = item.button(mtm) {
                let size = button.font().map(|font| font.pointSize()).unwrap_or(13.0);
                // SAFETY: AppKit defines this symbol as an immutable font-weight constant.
                let weight = unsafe { NSFontWeightRegular };
                let font = NSFont::monospacedDigitSystemFontOfSize_weight(size, weight);
                button.setFont(Some(&font));
            }
        }
    })
}

#[cfg(target_os = "macos")]
pub fn configure_overlay(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    let pointer = window.ns_window()?;
    // SAFETY: Called from setup on the main thread, immediately after building
    // the window. Tauri owns this NSWindow and keeps it alive for the entire call.
    unsafe {
        let native = &*pointer.cast::<NSWindow>();
        native.setCollectionBehavior(
            native.collectionBehavior()
                | NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
    }
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn idle_secs() -> u64 {
    0
}
