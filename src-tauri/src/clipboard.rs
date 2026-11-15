//! Clipboard history for the notch shelf.
//!
//! A background thread watches `GetClipboardSequenceNumber` (a cheap counter,
//! no clipboard access) and only opens the clipboard when it changes. Text,
//! images and copied files are kept in memory only (never written to disk) and
//! are lost when Orbix exits. Content that apps mark as private — password
//! managers set "ExcludeClipboardContentFromMonitorProcessing" or
//! "CanIncludeInClipboardHistory" = 0 — is never recorded.

use base64::Engine;
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};

const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;

const MAX_ENTRIES: usize = 30;
const MAX_TEXT_CHARS: usize = 200_000;
const MAX_IMAGE_BYTES_TOTAL: usize = 48 * 1024 * 1024;
const PREVIEW_CHARS: usize = 300;
const THUMB_SIZE: u32 = 240;
const POLL_MS: u64 = 400;

#[derive(Serialize, Clone, Debug)]
pub struct ClipSummary {
    pub id: u64,
    /// "text" | "image" | "files"
    pub kind: &'static str,
    pub preview: String,
    pub thumbnail: Option<String>,
    pub char_count: usize,
    pub file_count: usize,
    /// Unix ms of the copy.
    pub timestamp: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ShelfPaste {
    pub paths: Vec<String>,
    pub text: Option<String>,
}

enum ClipData {
    Text(String),
    Image { png: Vec<u8> },
    Files(Vec<String>),
}

struct Entry {
    summary: ClipSummary,
    data: ClipData,
    fingerprint: u64,
}

static HISTORY: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
static ENABLED: AtomicBool = AtomicBool::new(true);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Holds the clipboard open; closes it on drop.
struct OpenGuard;

impl OpenGuard {
    fn open() -> Option<Self> {
        // Another app may hold the clipboard for a moment right after copying.
        for _ in 0..6 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Some(OpenGuard);
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        None
    }
}

impl Drop for OpenGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

fn format_id(name: &str) -> u32 {
    let w = wide(name);
    unsafe { RegisterClipboardFormatW(PCWSTR(w.as_ptr())) }
}

fn available(format: u32) -> bool {
    format != 0 && unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// Bytes of a global-memory clipboard handle.
unsafe fn global_bytes(handle: HANDLE) -> Option<Vec<u8>> {
    let hglobal = HGLOBAL(handle.0);
    let size = unsafe { GlobalSize(hglobal) };
    let ptr = unsafe { GlobalLock(hglobal) } as *const u8;
    if ptr.is_null() || size == 0 {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, size) }.to_vec();
    let _ = unsafe { GlobalUnlock(hglobal) };
    Some(bytes)
}

/// True when the copying app asked monitors not to record this content.
fn is_private() -> bool {
    if available(format_id("ExcludeClipboardContentFromMonitorProcessing"))
        || available(format_id("Clipboard Viewer Ignore"))
    {
        return true;
    }
    let history_flag = format_id("CanIncludeInClipboardHistory");
    if available(history_flag) {
        if let Ok(handle) = unsafe { GetClipboardData(history_flag) } {
            if let Some(bytes) = unsafe { global_bytes(handle) } {
                if bytes.len() >= 4
                    && u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) == 0
                {
                    return true;
                }
            }
        }
    }
    false
}

fn read_files() -> Option<Vec<String>> {
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    let handle = unsafe { GetClipboardData(CF_HDROP) }.ok()?;
    let hdrop = HDROP(handle.0);
    let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
    let mut files = Vec::new();
    for i in 0..count.min(500) {
        let len = unsafe { DragQueryFileW(hdrop, i, None) } as usize;
        let mut buf = vec![0u16; len + 1];
        let written = unsafe { DragQueryFileW(hdrop, i, Some(&mut buf)) } as usize;
        if written > 0 {
            files.push(String::from_utf16_lossy(&buf[..written]));
        }
    }
    (!files.is_empty()).then_some(files)
}

fn read_text() -> Option<String> {
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) }.ok()?;
    let bytes = unsafe { global_bytes(handle) }?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    let text = String::from_utf16_lossy(&units);
    (!text.trim().is_empty() && text.chars().count() <= MAX_TEXT_CHARS).then_some(text)
}

/// Decodes a CF_DIB payload by giving it the file header a .bmp would have.
fn dib_to_rgba(dib: &[u8]) -> Option<image::RgbaImage> {
    if dib.len() < 40 {
        return None;
    }
    let header_size = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    let bit_count = u16::from_le_bytes(dib[14..16].try_into().ok()?) as usize;
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    let colors_used = u32::from_le_bytes(dib[32..36].try_into().ok()?) as usize;
    // BI_BITFIELDS with a plain 40-byte header carries three mask DWORDs.
    let masks = if compression == 3 && header_size == 40 {
        12
    } else {
        0
    };
    let palette = if bit_count <= 8 {
        (if colors_used > 0 {
            colors_used
        } else {
            1 << bit_count
        }) * 4
    } else {
        colors_used * 4
    };
    let offset = 14 + header_size + masks + palette;
    let file_size = (14 + dib.len()) as u32;
    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&(offset as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    let mut img = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp)
        .ok()?
        .to_rgba8();
    // Many apps leave the alpha byte of 32-bit DIBs at zero; treat as opaque.
    if img.pixels().all(|p| p[3] == 0) {
        img.pixels_mut().for_each(|p| p[3] = 255);
    }
    Some(img)
}

fn encode_png(img: &image::RgbaImage) -> Option<Vec<u8>> {
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}

fn thumbnail(img: &image::RgbaImage) -> Option<String> {
    let small = image::DynamicImage::ImageRgba8(img.clone())
        .thumbnail(THUMB_SIZE, THUMB_SIZE)
        .to_rgb8();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 78)
        .encode_image(&small)
        .ok()?;
    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&jpeg)
    ))
}

fn read_image() -> Option<(Vec<u8>, String)> {
    let handle = unsafe { GetClipboardData(CF_DIB) }.ok()?;
    let dib = unsafe { global_bytes(handle) }?;
    let img = dib_to_rgba(&dib)?;
    Some((encode_png(&img)?, thumbnail(&img)?))
}

fn fingerprint<T: Hash>(kind: u8, value: &T) -> u64 {
    let mut h = DefaultHasher::new();
    kind.hash(&mut h);
    value.hash(&mut h);
    h.finish()
}

fn preview_text(text: &str) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(PREVIEW_CHARS).collect()
}

/// Reads whatever is on the clipboard now, files first (Explorer also offers
/// text), then text (browsers also offer images), then a bitmap.
fn capture() -> Option<Entry> {
    let _guard = OpenGuard::open()?;
    if is_private() {
        return None;
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let timestamp = now_ms();
    if available(CF_HDROP) {
        let files = read_files()?;
        let names: Vec<&str> = files
            .iter()
            .map(|f| f.rsplit(['\\', '/']).next().unwrap_or(f))
            .collect();
        return Some(Entry {
            fingerprint: fingerprint(1, &files),
            summary: ClipSummary {
                id,
                kind: "files",
                preview: names.join(", "),
                thumbnail: None,
                char_count: 0,
                file_count: files.len(),
                timestamp,
            },
            data: ClipData::Files(files),
        });
    }
    if available(CF_UNICODETEXT) {
        let text = read_text()?;
        return Some(Entry {
            fingerprint: fingerprint(2, &text),
            summary: ClipSummary {
                id,
                kind: "text",
                preview: preview_text(&text),
                thumbnail: None,
                char_count: text.chars().count(),
                file_count: 0,
                timestamp,
            },
            data: ClipData::Text(text),
        });
    }
    if available(CF_DIB) {
        let (png, thumb) = read_image()?;
        return Some(Entry {
            fingerprint: fingerprint(3, &png),
            summary: ClipSummary {
                id,
                kind: "image",
                preview: String::new(),
                thumbnail: Some(thumb),
                char_count: 0,
                file_count: 0,
                timestamp,
            },
            data: ClipData::Image { png },
        });
    }
    None
}

/// Adds an entry on top; a repeat of an older entry moves it up instead.
fn record(entry: Entry) {
    let Ok(mut history) = HISTORY.lock() else {
        return;
    };
    history.retain(|e| e.fingerprint != entry.fingerprint);
    history.insert(0, entry);
    history.truncate(MAX_ENTRIES);
    // Cap memory held by images, dropping the oldest ones first.
    let mut image_bytes = 0usize;
    history.retain(|e| match &e.data {
        ClipData::Image { png } => {
            image_bytes += png.len();
            image_bytes <= MAX_IMAGE_BYTES_TOTAL
        }
        _ => true,
    });
}

fn summaries() -> Vec<ClipSummary> {
    HISTORY
        .lock()
        .map(|h| h.iter().map(|e| e.summary.clone()).collect())
        .unwrap_or_default()
}

fn notify(app: &AppHandle) {
    let _ = app.emit("clipboard-history-changed", summaries());
}

/// Starts the watcher thread. Call once at startup.
pub fn start_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_seq = unsafe { GetClipboardSequenceNumber() };
        loop {
            std::thread::sleep(std::time::Duration::from_millis(POLL_MS));
            let seq = unsafe { GetClipboardSequenceNumber() };
            if seq == last_seq {
                continue;
            }
            last_seq = seq;
            if !ENABLED.load(Ordering::Relaxed) {
                continue;
            }
            if let Some(entry) = capture() {
                record(entry);
                notify(&app);
            }
        }
    });
}

/// Moves `bytes` into a movable global block owned by the clipboard afterwards.
unsafe fn set_global(format: u32, bytes: &[u8]) -> Result<(), String> {
    let hglobal = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }.map_err(|e| e.to_string())?;
    let ptr = unsafe { GlobalLock(hglobal) } as *mut u8;
    if ptr.is_null() {
        return Err("Out of memory".into());
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
    let _ = unsafe { GlobalUnlock(hglobal) };
    unsafe { SetClipboardData(format, Some(HANDLE(hglobal.0))) }
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 32-bit bottom-up CF_DIB for an RGBA image.
fn rgba_to_dib(img: &image::RgbaImage) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let mut dib = Vec::with_capacity(40 + (w * h * 4) as usize);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(w as i32).to_le_bytes());
    dib.extend_from_slice(&(h as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    dib.extend_from_slice(&(w * h * 4).to_le_bytes());
    dib.extend_from_slice(&[0u8; 16]); // resolution and palette counts
    for y in (0..h).rev() {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            dib.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    dib
}

/// CF_HDROP payload: DROPFILES header + double-NUL-terminated wide paths.
fn hdrop_bytes(files: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&20u32.to_le_bytes()); // pFiles: offset of the list
    bytes.extend_from_slice(&[0u8; 8]); // pt
    bytes.extend_from_slice(&0u32.to_le_bytes()); // fNC
    bytes.extend_from_slice(&1u32.to_le_bytes()); // fWide
    for f in files {
        for unit in f.encode_utf16().chain(std::iter::once(0)) {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

fn write_to_clipboard(data: &ClipData) -> Result<(), String> {
    let _guard = OpenGuard::open().ok_or("The clipboard is busy, try again")?;
    unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
    match data {
        ClipData::Text(text) => {
            let bytes: Vec<u8> = text
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(|u| u.to_le_bytes())
                .collect();
            unsafe { set_global(CF_UNICODETEXT, &bytes) }
        }
        ClipData::Image { png } => {
            let img = image::load_from_memory(png)
                .map_err(|e| e.to_string())?
                .to_rgba8();
            unsafe { set_global(CF_DIB, &rgba_to_dib(&img)) }
        }
        ClipData::Files(files) => unsafe { set_global(CF_HDROP, &hdrop_bytes(files)) },
    }
}

fn with_entry<T>(id: u64, f: impl FnOnce(&ClipData) -> T) -> Result<T, String> {
    let history = HISTORY.lock().map_err(|e| e.to_string())?;
    let entry = history
        .iter()
        .find(|e| e.summary.id == id)
        .ok_or("That item is no longer in the clipboard history")?;
    Ok(f(&entry.data))
}

/// Saves image bytes under the app's cache so the shelf can hold them as files.
fn save_png(app: &AppHandle, png: &[u8]) -> Result<String, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("shelf-clips");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("clipboard-{}.png", now_ms()));
    std::fs::write(&path, png).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn to_shelf(app: &AppHandle, data: &ClipData) -> Result<ShelfPaste, String> {
    Ok(match data {
        ClipData::Files(files) => ShelfPaste {
            paths: files.clone(),
            text: None,
        },
        ClipData::Text(text) => ShelfPaste {
            paths: Vec::new(),
            text: Some(text.clone()),
        },
        ClipData::Image { png } => ShelfPaste {
            paths: vec![save_png(app, png)?],
            text: None,
        },
    })
}

#[tauri::command]
pub fn get_clipboard_history() -> Vec<ClipSummary> {
    summaries()
}

#[tauri::command]
pub async fn clipboard_restore(id: u64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let data = with_entry(id, |d| match d {
            ClipData::Text(t) => ClipData::Text(t.clone()),
            ClipData::Image { png } => ClipData::Image { png: png.clone() },
            ClipData::Files(f) => ClipData::Files(f.clone()),
        })?;
        write_to_clipboard(&data)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn clipboard_remove(app: AppHandle, id: u64) {
    if let Ok(mut history) = HISTORY.lock() {
        history.retain(|e| e.summary.id != id);
    }
    notify(&app);
}

#[tauri::command]
pub fn clipboard_clear(app: AppHandle) {
    if let Ok(mut history) = HISTORY.lock() {
        history.clear();
    }
    notify(&app);
}

#[tauri::command]
pub fn set_clipboard_history_enabled(app: AppHandle, enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        clipboard_clear(app);
    }
}

/// Shelf "Paste": a history entry (`id`) or, without one, whatever is on the
/// clipboard right now, as file paths (images are saved to PNG) or text.
#[tauri::command]
pub async fn clipboard_to_shelf(app: AppHandle, id: Option<u64>) -> Result<ShelfPaste, String> {
    tauri::async_runtime::spawn_blocking(move || match id {
        Some(id) => with_entry(id, |data| to_shelf(&app, data))?,
        None => {
            let entry =
                capture().ok_or("The clipboard is empty or holds nothing the shelf can keep")?;
            to_shelf(&app, &entry.data)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dib_round_trips_through_rgba() {
        let mut img = image::RgbaImage::new(3, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(2, 1, image::Rgba([0, 0, 255, 255]));
        let back = dib_to_rgba(&rgba_to_dib(&img)).unwrap();
        assert_eq!(back.dimensions(), (3, 2));
        assert_eq!(back.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(back.get_pixel(2, 1).0, [0, 0, 255, 255]);
    }

    #[test]
    fn zero_alpha_dibs_become_opaque() {
        let mut img = image::RgbaImage::new(2, 2);
        img.pixels_mut()
            .for_each(|p| *p = image::Rgba([10, 20, 30, 0]));
        let back = dib_to_rgba(&rgba_to_dib(&img)).unwrap();
        assert!(back.pixels().all(|p| p[3] == 255));
    }

    #[test]
    fn hdrop_layout() {
        let bytes = hdrop_bytes(&["C:\\a.txt".to_string()]);
        assert_eq!(&bytes[0..4], &20u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &1u32.to_le_bytes());
        assert_eq!(&bytes[bytes.len() - 4..], &[0, 0, 0, 0]);
    }

    #[test]
    fn previews_collapse_whitespace() {
        assert_eq!(preview_text("a\n\n  b\tc"), "a b c");
    }
}
