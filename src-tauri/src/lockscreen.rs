//! Orbix lock screen: a full-screen clock-and-widgets page shown after a while
//! idle or when locking from Orbix.
//!
//! Windows does not let apps draw on its real lock screen (a separate secure
//! desktop), so this page sits in front of the normal desktop instead. Enter or
//! a click outside the widgets — or anything that takes focus away from it —
//! hands over to the real Windows lock (`LockWorkStation`), which asks for the
//! PIN. Preview mode (from Settings) just closes without locking.
//!
//! Windows shows its own lock picture before the PIN box; the only switch for
//! that is the machine-wide `NoLockScreen` policy, which Settings can set (it
//! needs administrator approval).

use base64::Engine;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

const LABEL: &str = "lockscreen";
const MAX_WALLPAPER_EDGE: u32 = 2560;

static SHOWING: AtomicBool = AtomicBool::new(false);
static PREVIEW: AtomicBool = AtomicBool::new(false);
/// The page has been the foreground window since it last appeared. Losing
/// that afterwards means someone switched away (Alt+Tab, Win key).
static WAS_FOREGROUND: AtomicBool = AtomicBool::new(false);
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
    WAS_FOREGROUND.store(false, Ordering::SeqCst);
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
    crate::perf::set_low_memory(&win, false);
    let _ = win.show();
    let _ = win.set_always_on_top(true);
    let _ = win.set_focus();
    if let Ok(hwnd) = win.hwnd() {
        force_foreground(hwnd);
    }
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

/// Windows refuses SetForegroundWindow to background apps (the idle case);
/// joining the current foreground thread's input queue lifts that, so Enter
/// reaches the page instead of whatever app was in front.
fn force_foreground(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground == hwnd {
            return;
        }
        let ours = GetCurrentThreadId();
        let theirs = GetWindowThreadProcessId(foreground, None);
        let attached =
            theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(ours, theirs, false);
        }
    }
}

/// Shows the page after the configured idle time. While it shows, keeps it in
/// front, and locks if something else takes the foreground (Alt+Tab, Win key)
/// — the page itself only closes on Enter or a click.
pub fn start_idle_watch(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let Some(last_input) = last_input_tick() else {
            continue;
        };
        if SHOWING.load(Ordering::SeqCst) {
            let Some(hwnd) = app.get_webview_window(LABEL).and_then(|w| w.hwnd().ok()) else {
                continue;
            };
            let foreground =
                unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
            if foreground == hwnd {
                WAS_FOREGROUND.store(true, Ordering::SeqCst);
            } else if WAS_FOREGROUND.load(Ordering::SeqCst) {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || dismiss(&handle));
            } else {
                force_foreground(hwnd);
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

const PERSONALIZATION_POLICY: &str = r"SOFTWARE\Policies\Microsoft\Windows\Personalization";

/// Whether Windows skips its lock picture and opens the PIN box directly
/// (machine policy `NoLockScreen` = 1).
#[tauri::command]
pub fn lock_curtain_skipped() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};
    let key = to_wide(PERSONALIZATION_POLICY);
    let value = to_wide("NoLockScreen");
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            windows::core::PCWSTR(key.as_ptr()),
            windows::core::PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    status.is_ok() && data == 1
}

/// Sets or clears `NoLockScreen` through an elevated `reg.exe` (Windows asks
/// for administrator approval). Returns the resulting state.
#[tauri::command]
pub async fn set_lock_curtain_skipped(skip: bool) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
        use windows::Win32::System::Threading::WaitForSingleObject;
        use windows::Win32::UI::Shell::{
            ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
        };
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

        let key = format!(r"HKLM\{PERSONALIZATION_POLICY}");
        let args = if skip {
            format!(r#"add "{key}" /v NoLockScreen /t REG_DWORD /d 1 /f"#)
        } else {
            format!(r#"delete "{key}" /v NoLockScreen /f"#)
        };
        let verb = to_wide("runas");
        let file = to_wide("reg.exe");
        let params = to_wide(&args);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: windows::core::PCWSTR(verb.as_ptr()),
            lpFile: windows::core::PCWSTR(file.as_ptr()),
            lpParameters: windows::core::PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        unsafe {
            ShellExecuteExW(&mut info)
                .map_err(|_| "Windows didn't get administrator approval".to_string())?;
            if !info.hProcess.is_invalid() {
                if WaitForSingleObject(info.hProcess, 15_000) != WAIT_OBJECT_0 {
                    let _ = CloseHandle(info.hProcess);
                    return Err("The registry change timed out".into());
                }
                let _ = CloseHandle(info.hProcess);
            }
        }
        // Deleting a value that isn't there "fails" but leaves the wanted state.
        Ok(lock_curtain_skipped())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
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
