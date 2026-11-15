//! RAM overview and "free up memory" tools for the notch Performance panel.
//!
//! Trimming working sets (EmptyWorkingSet) moves idle pages out of processes so
//! Windows reports more available memory right away; pages a process touches
//! again are faulted back in. It helps when RAM is nearly full and something new
//! needs room, but the lasting win is closing heavy apps, which the panel also
//! offers per app.

use serde::Serialize;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::System::ProcessStatus::{
    K32EmptyWorkingSet, K32EnumProcesses, K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
};

const MB: u64 = 1024 * 1024;
const TOP_APPS: usize = 6;

/// Processes that must never be offered for closing: the shell, session and
/// security infrastructure, and WebView2 (which hosts Orbix's own UI).
const PROTECTED: &[&str] = &[
    "system",
    "registry",
    "memory compression",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "lsaiso.exe",
    "svchost.exe",
    "dwm.exe",
    "explorer.exe",
    "sihost.exe",
    "fontdrvhost.exe",
    "ctfmon.exe",
    "audiodg.exe",
    "msmpeng.exe",
    "securityhealthservice.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "searchhost.exe",
    "textinputhost.exe",
    "runtimebroker.exe",
    "taskmgr.exe",
    "msedgewebview2.exe",
    "orbix.exe",
];

#[derive(Serialize, Clone, Debug)]
pub struct AppMemoryUsage {
    /// Executable file name, e.g. `chrome.exe`; also the key for `close_app`.
    pub name: String,
    pub memory_mb: u64,
    pub process_count: u32,
}

#[derive(Serialize, Clone, Debug)]
pub struct MemoryReport {
    pub total_mb: u64,
    pub available_mb: u64,
    pub used_percent: u32,
    pub top_apps: Vec<AppMemoryUsage>,
}

#[derive(Serialize, Clone, Debug)]
pub struct OptimizeResult {
    pub freed_mb: u64,
    pub processes_trimmed: u32,
    pub available_mb: u64,
    pub used_percent: u32,
}

#[derive(Serialize, Clone, Debug)]
pub struct CloseAppResult {
    /// Processes of that app still running after the attempt.
    pub remaining: u32,
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn open(
    pid: u32,
    access: windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS,
) -> Option<OwnedHandle> {
    unsafe { OpenProcess(access, false, pid).ok().map(OwnedHandle) }
}

fn memory_status() -> (u64, u64, u32) {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe {
        if GlobalMemoryStatusEx(&mut status).is_err() {
            return (0, 0, 0);
        }
    }
    (
        status.ullTotalPhys / MB,
        status.ullAvailPhys / MB,
        status.dwMemoryLoad,
    )
}

fn all_pids() -> Vec<u32> {
    let mut pids = vec![0u32; 4096];
    let mut needed = 0u32;
    loop {
        let bytes = (pids.len() * std::mem::size_of::<u32>()) as u32;
        let ok = unsafe { K32EnumProcesses(pids.as_mut_ptr(), bytes, &mut needed) }.as_bool();
        if !ok {
            return Vec::new();
        }
        // A full buffer may mean truncation: grow and retry.
        if needed < bytes || pids.len() >= 65_536 {
            pids.truncate(needed as usize / std::mem::size_of::<u32>());
            return pids;
        }
        pids.resize(pids.len() * 2, 0);
    }
}

fn process_name(handle: &OwnedHandle) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .ok()?;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    path.rsplit(['\\', '/']).next().map(|s| s.to_string())
}

fn working_set_bytes(handle: &OwnedHandle) -> u64 {
    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    let ok = unsafe { K32GetProcessMemoryInfo(handle.0, &mut counters, size) }.as_bool();
    if ok {
        counters.WorkingSetSize as u64
    } else {
        0
    }
}

fn is_protected(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    PROTECTED.contains(&lower.as_str())
}

/// Groups processes by executable name and sums their working sets.
fn aggregate(samples: &[(String, u64)]) -> Vec<AppMemoryUsage> {
    let mut apps: Vec<AppMemoryUsage> = Vec::new();
    for (name, bytes) in samples {
        if let Some(app) = apps.iter_mut().find(|a| a.name.eq_ignore_ascii_case(name)) {
            app.memory_mb += bytes / MB;
            app.process_count += 1;
        } else {
            apps.push(AppMemoryUsage {
                name: name.clone(),
                memory_mb: bytes / MB,
                process_count: 1,
            });
        }
    }
    apps.sort_by_key(|a| std::cmp::Reverse(a.memory_mb));
    apps
}

fn pids_named(name: &str) -> Vec<u32> {
    all_pids()
        .into_iter()
        .filter(|&pid| {
            open(pid, PROCESS_QUERY_LIMITED_INFORMATION)
                .and_then(|h| process_name(&h))
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .collect()
}

pub fn report() -> MemoryReport {
    let own_pid = unsafe { GetCurrentProcessId() };
    let samples: Vec<(String, u64)> = all_pids()
        .into_iter()
        .filter(|&pid| pid != 0 && pid != own_pid)
        .filter_map(|pid| {
            let handle = open(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
            let name = process_name(&handle)?;
            if is_protected(&name) {
                return None;
            }
            Some((name, working_set_bytes(&handle)))
        })
        .collect();
    let mut top_apps = aggregate(&samples);
    top_apps.truncate(TOP_APPS);
    let (total_mb, available_mb, used_percent) = memory_status();
    MemoryReport {
        total_mb,
        available_mb,
        used_percent,
        top_apps,
    }
}

/// Trims the working set of every process this user may modify.
pub fn optimize() -> OptimizeResult {
    let (_, before_mb, _) = memory_status();
    let own_pid = unsafe { GetCurrentProcessId() };
    let mut trimmed = 0u32;
    for pid in all_pids() {
        if pid == 0 || pid == own_pid {
            continue;
        }
        if let Some(handle) = open(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA) {
            if unsafe { K32EmptyWorkingSet(handle.0) }.as_bool() {
                trimmed += 1;
            }
        }
    }
    // The memory manager moves trimmed pages to the standby list asynchronously.
    std::thread::sleep(std::time::Duration::from_millis(400));
    let (_, after_mb, used_percent) = memory_status();
    OptimizeResult {
        freed_mb: after_mb.saturating_sub(before_mb),
        processes_trimmed: trimmed,
        available_mb: after_mb,
        used_percent,
    }
}

unsafe extern "system" fn post_close_to_pid(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
    };
    let target = lparam.0 as u32;
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == target && IsWindowVisible(hwnd).as_bool() {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    windows::core::BOOL(1)
}

/// Asks every window of the app to close (like clicking X, so apps can save
/// work). With `force`, terminates the processes instead.
pub fn close_app(name: &str, force: bool) -> Result<CloseAppResult, String> {
    if is_protected(name) {
        return Err(format!(
            "{name} is part of Windows or Orbix and can't be closed here"
        ));
    }
    let pids = pids_named(name);
    if pids.is_empty() {
        return Ok(CloseAppResult { remaining: 0 });
    }
    for &pid in &pids {
        if force {
            if let Some(handle) = open(pid, PROCESS_TERMINATE) {
                unsafe {
                    let _ = TerminateProcess(handle.0, 1);
                }
            }
        } else {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
                    Some(post_close_to_pid),
                    LPARAM(pid as isize),
                );
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(if force {
        500
    } else {
        1500
    }));
    Ok(CloseAppResult {
        remaining: pids_named(name).len() as u32,
    })
}

#[tauri::command]
pub async fn get_memory_report() -> Result<MemoryReport, String> {
    tauri::async_runtime::spawn_blocking(report)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn optimize_memory() -> Result<OptimizeResult, String> {
    tauri::async_runtime::spawn_blocking(optimize)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn close_heavy_app(name: String, force: bool) -> Result<CloseAppResult, String> {
    tauri::async_runtime::spawn_blocking(move || close_app(&name, force))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_by_name_case_insensitively() {
        let samples = vec![
            ("chrome.exe".to_string(), 300 * MB),
            ("Chrome.exe".to_string(), 200 * MB),
            ("code.exe".to_string(), 400 * MB),
        ];
        let apps = aggregate(&samples);
        assert_eq!(apps[0].name, "chrome.exe");
        assert_eq!(apps[0].memory_mb, 500);
        assert_eq!(apps[0].process_count, 2);
        assert_eq!(apps[1].memory_mb, 400);
    }

    #[test]
    fn protects_shell_and_own_ui() {
        assert!(is_protected("Explorer.EXE"));
        assert!(is_protected("msedgewebview2.exe"));
        assert!(!is_protected("chrome.exe"));
    }
}
