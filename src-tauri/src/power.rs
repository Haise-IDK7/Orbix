use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[link(name = "user32")]
extern "system" {
    fn LockWorkStation() -> windows::core::BOOL;
}

#[tauri::command]
pub fn power_shutdown() -> Result<(), String> {
    Command::new("shutdown")
        .args(["/s", "/t", "0"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to initiate shutdown: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn power_restart() -> Result<(), String> {
    Command::new("shutdown")
        .args(["/r", "/t", "0"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to initiate restart: {e}"))?;
    Ok(())
}

/// Lets this process use SetSuspendState (standard users hold the privilege,
/// but it starts disabled).
fn enable_shutdown_privilege() {
    use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token).is_err() {
            return;
        }
        let mut luid = LUID::default();
        if LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut luid).is_ok() {
            let privileges = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: luid,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };
            let _ = AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None);
        }
        let _ = CloseHandle(token);
    }
}

/// Sleep (suspend to RAM). Calls SetSuspendState directly: the old
/// `rundll32 powrprof.dll,SetSuspendState 0,1,0` passes rundll32's own
/// arguments instead of 0,1,0, so on PCs with hibernation enabled it
/// hibernated. On Modern Standby PCs, which don't support that call, turning
/// the display off is how Windows enters standby.
#[tauri::command]
pub fn power_sleep() -> Result<(), String> {
    // SetSuspendState only returns after the PC wakes up; keep it off the
    // main thread.
    std::thread::spawn(|| unsafe {
        enable_shutdown_privilege();
        if !windows::Win32::System::Power::SetSuspendState(false, false, false) {
            use windows::Win32::Foundation::{LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{
                PostMessageW, HWND_BROADCAST, SC_MONITORPOWER, WM_SYSCOMMAND,
            };
            const MONITOR_OFF: isize = 2;
            let _ = PostMessageW(
                Some(HWND_BROADCAST),
                WM_SYSCOMMAND,
                WPARAM(SC_MONITORPOWER as usize),
                LPARAM(MONITOR_OFF),
            );
        }
    });
    Ok(())
}

#[tauri::command]
pub fn power_lock(app: tauri::AppHandle) -> Result<(), String> {
    // The Orbix lock screen shows first and locks Windows on the first touch.
    if crate::lockscreen::replaces_lock_button(&app) {
        return crate::lockscreen::show(&app, false);
    }
    unsafe {
        let res = LockWorkStation();
        if res.as_bool() {
            return Ok(());
        }
    }

    // Fallback via rundll32
    Command::new("rundll32.exe")
        .args(["user32.dll,LockWorkStation"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to lock workstation: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn power_switch_user() -> Result<(), String> {
    Command::new("tsdiscon")
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to switch user: {e}"))?;
    Ok(())
}
