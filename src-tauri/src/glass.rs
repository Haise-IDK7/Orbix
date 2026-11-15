//! Backdrop for the "glass" style of the notch and dock.
//!
//! WebView2 can't blur what is behind its window (`backdrop-filter` only sees
//! the page itself), and DWM acrylic would cover the whole rectangular window
//! instead of the rounded notch/dock. So the glass is drawn the way in-app
//! acrylic works: a small, pre-blurred copy of the desktop wallpaper, aligned
//! to the screen by the frontend. Blurring once here keeps the webview free of
//! expensive CSS filters.

use base64::Engine;
use serde::Serialize;
use std::sync::Mutex;

const BACKDROP_WIDTH: u32 = 480;
/// Gaussian sigma at 480 px wide ≈ 40 px of blur on a 1920 px screen.
const BLUR_SIGMA: f32 = 10.0;

#[derive(Serialize, Clone, Debug)]
pub struct GlassBackdrop {
    /// Stable for the same wallpaper file, so the frontend can skip updates.
    pub key: String,
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

static CACHE: Mutex<Option<GlassBackdrop>> = Mutex::new(None);

fn render(path: &str) -> Result<GlassBackdrop, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let key = format!("{path}|{}|{mtime}", meta.len());
    if let Ok(guard) = CACHE.lock() {
        if let Some(cached) = guard.as_ref().filter(|c| c.key == key) {
            return Ok(cached.clone());
        }
    }

    // The slideshow/"TranscodedWallpaper" file has no extension, so sniff it.
    let image = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;
    let small = image.resize(
        BACKDROP_WIDTH,
        BACKDROP_WIDTH * 4,
        image::imageops::FilterType::Triangle,
    );
    let blurred =
        image::DynamicImage::ImageRgba8(image::imageops::fast_blur(&small.to_rgba8(), BLUR_SIGMA))
            .to_rgb8();

    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80)
        .encode_image(&blurred)
        .map_err(|e| e.to_string())?;
    let backdrop = GlassBackdrop {
        key,
        data_url: format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&jpeg)
        ),
        width: blurred.width(),
        height: blurred.height(),
    };
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(backdrop.clone());
    }
    Ok(backdrop)
}

/// Blurred copy of the current wallpaper, or `None` for a solid-color desktop.
#[tauri::command]
pub async fn get_glass_backdrop() -> Result<Option<GlassBackdrop>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = crate::wallpaper::get_current_wallpaper().unwrap_or_default();
        if path.trim().is_empty() || !std::path::Path::new(&path).is_file() {
            return Ok(None);
        }
        render(&path).map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}
