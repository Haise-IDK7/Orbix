//! Orbix lock screen: a full-screen clock-and-widgets page shown after a while
//! idle or when locking from Orbix.
//!
//! Windows does not let apps draw on its real lock screen (a separate secure
//! desktop), so this page sits in front of the normal desktop instead. To stay
//! safe, any interaction with it — or anything that takes focus away from it —
//! hands over to the real Windows lock (`LockWorkStation`), which asks for the
//! PIN. Preview mode (from Settings) just closes without locking.

use base64::Engine;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

const LABEL: &str = "lockscreen";
const MAX_WALLPAPER_EDGE: u32 = 2560;

static SHOWING: AtomicBool = AtomicBool::new(false);
static PREVIEW: AtomicBool = AtomicBool::new(false);
/// GetTickCount value when the page appeared.
static SHOWN_AT: AtomicU32 = AtomicU32::new(0);
/// Last-input tick already "used" by a show or dismiss. The idle timer only
/// fires again after new input, so it can't reopen the page behind the
/// Windows lock it just handed over to.
static HANDLED_INPUT: AtomicU32 = AtomicU32::new(0);
static HIDDEN_WINDOWS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn setting(app: &AppHandle, key: &str, default: &str) -> String {
    crate::utils::get_setting_str(app, key).unwrap_or_else(|| default.to_string())
}

pub fn enabled(app: &AppHandle) -> bool {
    setting(app, "bloom-lock-enabled", "true") != "false"
}

/// Whether Orbix's own Lock buttons should show this page first.
pub fn replaces_lock_button(app: &AppHandle) -> bool {
    enabled(app) && setting(app, "bloom-lock-on-lock", "true") != "false"
}

fn tick_count() -> u32 {
    unsafe { windows::Win32::System::SystemInformation::GetTickCount() }
}

fn last_input_tick() -> Option<u32> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe { GetLastInputInfo(&mut info) }
        .as_bool()
        .then_some(info.dwTime)
}

/// A full-screen game/video/presentation is in front.
fn fullscreen_app_active() -> bool {
    use windows::Win32::UI::Shell::{
        SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
        QUNS_RUNNING_D3D_FULL_SCREEN,
    };
    matches!(
        unsafe { SHQueryUserNotificationState() },
        Ok(s) if s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE
    )
}

/// Some app (a video player, a call) asked Windows to keep the display on.
fn display_requested() -> bool {
    use windows::Win32::System::Power::{CallNtPowerInformation, SystemExecutionState};
    const ES_DISPLAY_REQUIRED: u32 = 0x2;
    let mut state = 0u32;
    let status = unsafe {
        CallNtPowerInformation(
            SystemExecutionState,
            None,
            0,
            Some(&mut state as *mut u32 as *mut _),
            std::mem::size_of::<u32>() as u32,
        )
    };
    status.is_ok() && state & ES_DISPLAY_REQUIRED != 0
}

pub fn show(app: &AppHandle, preview: bool) -> Result<(), String> {
    let win = app
        .get_webview_window(LABEL)
        .ok_or("The lock screen window is missing")?;
    if SHOWING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    PREVIEW.store(preview, Ordering::SeqCst);
    SHOWN_AT.store(tick_count(), Ordering::SeqCst);
    if let Some(input) = last_input_tick() {
        HANDLED_INPUT.store(input, Ordering::SeqCst);
    }

    if let Ok(Some(monitor)) = win.primary_monitor() {
        let _ = win.set_position(*monitor.position());
        let _ = win.set_size(*monitor.size());
    }
    // The notch and dock are topmost too; keep them from drawing on top.
    if let Ok(mut hidden) = HIDDEN_WINDOWS.lock() {
        hidden.clear();
        for label in ["main", "dock"] {
            if let Some(w) = app.get_webview_window(label) {
                if w.is_visible().unwrap_or(false) && w.hide().is_ok() {
                    hidden.push(label.to_string());
                }
            }
        }
    }
    let _ = win.show();
    let _ = win.set_always_on_top(true);
    let _ = win.set_focus();
    let _ = app.emit_to(LABEL, "lockscreen-show", preview);
    Ok(())
}

pub fn dismiss(app: &AppHandle) {
    if !SHOWING.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Some(input) = last_input_tick() {
        HANDLED_INPUT.store(input, Ordering::SeqCst);
    }
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.hide();
    }
    if let Ok(mut hidden) = HIDDEN_WINDOWS.lock() {
        for label in hidden.drain(..) {
            if let Some(w) = app.get_webview_window(&label) {
                let _ = w.show();
            }
        }
    }
    if !PREVIEW.load(Ordering::SeqCst) {
        unsafe {
            let _ = windows::Win32::System::Shutdown::LockWorkStation();
        }
    }
}

/// Shows the page after the configured idle time, and treats any input made
/// while it shows (even if another window kept keyboard focus) as a dismiss.
pub fn start_idle_watch(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let Some(last_input) = last_input_tick() else {
            continue;
        };
        if SHOWING.load(Ordering::SeqCst) {
            // Ignore input in the first second (the click/keypress that showed it).
            let shown_at = SHOWN_AT.load(Ordering::SeqCst);
            if last_input.wrapping_sub(shown_at) < u32::MAX / 2
                && last_input.wrapping_sub(shown_at) > 1000
            {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || dismiss(&handle));
            }
            continue;
        }
        let minutes: u64 = setting(&app, "bloom-lock-idle", "5").parse().unwrap_or(0);
        if !enabled(&app) || minutes == 0 || last_input == HANDLED_INPUT.load(Ordering::SeqCst) {
            continue;
        }
        let idle_ms = tick_count().wrapping_sub(last_input) as u64;
        if idle_ms < minutes * 60_000 || fullscreen_app_active() || display_requested() {
            continue;
        }
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = show(&handle, false);
        });
    });
}

#[tauri::command]
pub fn show_lock_screen(app: AppHandle, preview: bool) -> Result<(), String> {
    show(&app, preview)
}

#[tauri::command]
pub fn dismiss_lock_screen(app: AppHandle) {
    dismiss(&app);
}

#[derive(Serialize, Clone)]
pub struct LockWallpaper {
    pub key: String,
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

static WALLPAPER_CACHE: Mutex<Option<LockWallpaper>> = Mutex::new(None);

/// The lock screen background: `path`, or the desktop wallpaper when empty,
/// scaled to at most 2560 px on its long edge.
#[tauri::command]
pub async fn get_lock_wallpaper(path: Option<String>) -> Result<Option<LockWallpaper>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = path
            .filter(|p| !p.trim().is_empty())
            .or_else(|| crate::wallpaper::get_current_wallpaper().ok())
            .unwrap_or_default();
        let Ok(meta) = std::fs::metadata(&path) else {
            return Ok(None);
        };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let key = format!("{path}|{}|{mtime}", meta.len());
        if let Some(hit) = WALLPAPER_CACHE
            .lock()
            .ok()
            .and_then(|c| c.clone())
            .filter(|c| c.key == key)
        {
            return Ok(Some(hit));
        }
        let img = image::ImageReader::open(&path)
            .map_err(|e| e.to_string())?
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .decode()
            .map_err(|e| e.to_string())?;
        let img = if img.width().max(img.height()) > MAX_WALLPAPER_EDGE {
            img.resize(
                MAX_WALLPAPER_EDGE,
                MAX_WALLPAPER_EDGE,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            img
        };
        let rgb = img.to_rgb8();
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode_image(&rgb)
            .map_err(|e| e.to_string())?;
        let result = LockWallpaper {
            key,
            data_url: format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&jpeg)
            ),
            width: rgb.width(),
            height: rgb.height(),
        };
        if let Ok(mut cache) = WALLPAPER_CACHE.lock() {
            *cache = Some(result.clone());
        }
        Ok(Some(result))
    })
    .await
    .map_err(|e| e.to_string())?
}
