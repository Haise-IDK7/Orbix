//! Keeps Orbix's idle windows light.
//!
//! Settings and the lock screen live in hidden WebView2 windows so they open
//! instantly. While hidden, their WebView2 is told to use a low memory target
//! (`ICoreWebView2_19::SetMemoryUsageTargetLevel`), which lets it drop caches
//! and page out what it doesn't need; they go back to normal when shown. The
//! notch, dock and overlay stay normal: they're on screen or appear in a flash.

use tauri::{AppHandle, Manager, WebviewWindow};

/// Windows that spend most of their time hidden.
const MOSTLY_HIDDEN: [&str; 2] = ["settings", "lockscreen"];
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(3);

/// Low memory target while hidden, normal while shown. Silently does nothing
/// on WebView2 runtimes older than the API (before 1.0.2210).
pub fn set_low_memory(window: &WebviewWindow, low: bool) {
    let _ = window.with_webview(move |webview| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        };
        use windows::core::Interface;
        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        if let Ok(core19) = core.cast::<ICoreWebView2_19>() {
            let _ = core19.SetMemoryUsageTargetLevel(if low {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            });
        }
    });
}

/// Puts the mostly-hidden windows on the low target and follows their
/// visibility (they're hidden from several places: close buttons, Escape,
/// the tray), so every path is covered.
pub fn start_memory_tuning(app: AppHandle) {
    std::thread::spawn(move || {
        let mut applied: [Option<bool>; MOSTLY_HIDDEN.len()] = [None; MOSTLY_HIDDEN.len()];
        loop {
            for (i, label) in MOSTLY_HIDDEN.iter().enumerate() {
                let Some(window) = app.get_webview_window(label) else {
                    continue;
                };
                let low = !window.is_visible().unwrap_or(true);
                if applied[i] != Some(low) {
                    applied[i] = Some(low);
                    set_low_memory(&window, low);
                }
            }
            std::thread::sleep(CHECK_EVERY);
        }
    });
}
