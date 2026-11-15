//! Icons for packaged (MSIX / Microsoft Store) apps, read from the package.
//!
//! Asking the shell for an AppsFolder item's image (`IShellItemImageFactory`)
//! can return Windows' generic "app" glyph instead of the real logo, which is
//! what WhatsApp and Spotify showed in the dock. Here the logo is taken from
//! the package itself: locate the install folder with the documented AppModel
//! API, read the app's `VisualElements` from `AppxManifest.xml`, pick the
//! best qualified asset (`targetsize-256_altform-unplated` first), and, for
//! logos designed to sit on a colored tile, paint that tile like Start does.

use base64::Engine;
use std::path::{Path, PathBuf};

const ICON_SIZE: u32 = 128;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

/// Icon for an AppUserModelID such as `5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App`.
pub fn icon_for_aumid(aumid: &str) -> Option<String> {
    let id = aumid.trim().trim_start_matches("shell:AppsFolder\\");
    let (family, app_id) = id.split_once('!')?;
    let root = package_root_for_family(family)?;
    icon_from_package(&root, AppSelector::Id(app_id))
}

/// Icon for an executable inside `...\WindowsApps\<PackageFullName>\...`.
pub fn icon_for_packaged_exe(exe_path: &str) -> Option<String> {
    let lower = exe_path.to_lowercase();
    let marker = "\\windowsapps\\";
    let start = lower.find(marker)? + marker.len();
    let rest = &exe_path[start..];
    let (folder, relative_exe) = rest.split_once('\\')?;
    let root = Path::new(&exe_path[..start]).join(folder);
    icon_from_package(&root, AppSelector::Executable(relative_exe))
}

enum AppSelector<'a> {
    Id(&'a str),
    Executable(&'a str),
}

fn icon_from_package(root: &Path, selector: AppSelector) -> Option<String> {
    let manifest = std::fs::read_to_string(root.join("AppxManifest.xml")).ok()?;
    let visuals = manifest_visuals(&manifest, &selector)?;
    let (asset, unplated) = [
        (visuals.square44.as_deref(), 44u32),
        (visuals.square150.as_deref(), 150u32),
    ]
    .into_iter()
    .filter_map(|(logo, base)| best_asset(root, logo?, base))
    .max_by_key(|(_, unplated, size)| (*unplated, *size))
    .map(|(path, unplated, _)| (path, unplated))?;
    render_icon(&asset, unplated, visuals.background)
}

#[derive(Debug, PartialEq)]
struct Visuals {
    square44: Option<String>,
    square150: Option<String>,
    /// `None` for "transparent" or a missing color.
    background: Option<[u8; 3]>,
}

/// Value of `name="..."` inside one tag's text.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')?;
    Some(tag[start..start + end].to_string())
}

/// The opening tag that starts at `start` (up to its closing `>`).
fn tag_at(text: &str, start: usize) -> &str {
    let end = text[start..]
        .find('>')
        .map_or(text.len(), |e| start + e + 1);
    &text[start..end]
}

fn manifest_visuals(manifest: &str, selector: &AppSelector) -> Option<Visuals> {
    let mut search_from = 0;
    let mut first: Option<Visuals> = None;
    while let Some(offset) = manifest[search_from..].find("<Application ") {
        let start = search_from + offset;
        let open_tag = tag_at(manifest, start);
        let block_end = manifest[start..]
            .find("</Application>")
            .map_or(manifest.len(), |e| start + e);
        search_from = start + open_tag.len();

        let matches = match selector {
            AppSelector::Id(id) => attribute(open_tag, "Id").as_deref() == Some(*id),
            AppSelector::Executable(exe) => attribute(open_tag, "Executable")
                .is_some_and(|e| e.replace('/', "\\").eq_ignore_ascii_case(exe)),
        };

        let block = &manifest[start..block_end];
        let visuals = block.find("VisualElements ").map(|v| {
            let tag = tag_at(block, v);
            Visuals {
                square44: attribute(tag, "Square44x44Logo"),
                square150: attribute(tag, "Square150x150Logo"),
                background: attribute(tag, "BackgroundColor").and_then(|c| parse_color(&c)),
            }
        });
        if matches {
            return visuals;
        }
        if first.is_none() {
            first = visuals;
        }
    }
    // A single-app package whose Executable didn't match exactly (e.g. the
    // running exe is a helper of the declared one) still has the right logo.
    match selector {
        AppSelector::Executable(_) => first,
        AppSelector::Id(_) => None,
    }
}

fn parse_color(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None; // "transparent" and named colors
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// Scores one candidate file name (`Logo.targetsize-256_altform-unplated.png`)
/// or folder-qualified path (`scale-200\Logo.png`). `None` = not usable.
fn score_asset(relative: &str, base_size: u32) -> Option<(bool, u32)> {
    let lower = relative.to_lowercase();
    if lower.contains("contrast-") {
        return None; // high-contrast variants
    }
    let mut size = base_size;
    for part in lower.split(['.', '_', '\\', '/']) {
        if let Some(n) = part.strip_prefix("targetsize-") {
            size = n.parse().ok()?;
        } else if let Some(n) = part.strip_prefix("scale-") {
            size = base_size * n.parse::<u32>().ok()? / 100;
        }
    }
    let unplated = lower.contains("altform-unplated");
    // Light-unplated assets are tuned for light backgrounds; usable, but last.
    let light_only = lower.contains("altform-lightunplated");
    Some((unplated && !light_only, size.min(256)))
}

fn best_asset(root: &Path, logo: &str, base_size: u32) -> Option<(PathBuf, bool, u32)> {
    let logo_path = root.join(
        logo.replace('/', "\\")
            .replace('\\', std::path::MAIN_SEPARATOR_STR),
    );
    let dir = logo_path.parent()?;
    let stem = logo_path.file_stem()?.to_str()?.to_lowercase();
    let ext = logo_path.extension()?.to_str()?.to_lowercase();

    let mut best: Option<(PathBuf, bool, u32)> = None;
    let mut stack = vec![(dir.to_path_buf(), 0u8)];
    while let Some((folder, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth < 2 {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let lower = name.to_lowercase();
            let same_logo = lower == format!("{stem}.{ext}")
                || (lower.starts_with(&format!("{stem}.")) && lower.ends_with(&format!(".{ext}")));
            if !same_logo {
                continue;
            }
            let Ok(relative) = path.strip_prefix(dir) else {
                continue;
            };
            let relative = relative.to_string_lossy();
            let Some((unplated, size)) = score_asset(&relative, base_size) else {
                continue;
            };
            if best
                .as_ref()
                .is_none_or(|(_, u, s)| (unplated, size) > (*u, *s))
            {
                best = Some((path, unplated, size));
            }
        }
    }
    best
}

/// True when every visible pixel is near-white: a glyph meant for a colored tile.
fn is_light_glyph(img: &image::RgbaImage) -> bool {
    let mut visible = 0u32;
    let mut light = 0u32;
    for p in img.pixels() {
        if p[3] < 40 {
            continue;
        }
        visible += 1;
        if p[0] > 215 && p[1] > 215 && p[2] > 215 {
            light += 1;
        }
    }
    visible > 0 && light * 100 >= visible * 90
}

/// Rounded tile in `color` with the logo composited on top.
fn plate(logo: &image::RgbaImage, color: [u8; 3]) -> image::RgbaImage {
    let size = ICON_SIZE as f32;
    let radius = size * 0.22;
    let mut out = image::RgbaImage::new(ICON_SIZE, ICON_SIZE);
    for (x, y, px) in out.enumerate_pixels_mut() {
        // Signed distance to the rounded rectangle, for an anti-aliased edge.
        let cx = (x as f32 + 0.5 - size / 2.0).abs() - (size / 2.0 - radius);
        let cy = (y as f32 + 0.5 - size / 2.0).abs() - (size / 2.0 - radius);
        let outside = (cx.max(0.0).powi(2) + cy.max(0.0).powi(2)).sqrt() + cx.max(cy).min(0.0);
        let coverage = (radius - outside + 0.5).clamp(0.0, 1.0);
        *px = image::Rgba([color[0], color[1], color[2], (coverage * 255.0) as u8]);
    }
    for (x, y, src) in logo.enumerate_pixels() {
        let dst = out.get_pixel_mut(x, y);
        let a = src[3] as f32 / 255.0;
        if a == 0.0 || dst[3] == 0 {
            continue;
        }
        for c in 0..3 {
            dst[c] = (src[c] as f32 * a + dst[c] as f32 * (1.0 - a)).round() as u8;
        }
    }
    out
}

fn render_icon(asset: &Path, unplated: bool, background: Option<[u8; 3]>) -> Option<String> {
    let decoded = image::ImageReader::open(asset)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let logo = decoded
        .resize_exact(ICON_SIZE, ICON_SIZE, image::imageops::FilterType::Lanczos3)
        .to_rgba8();

    let tile = if unplated {
        None
    } else {
        // Plated logos assume the manifest's tile color behind them. Without
        // one, only a white glyph needs a backing to stay visible.
        background.or_else(|| is_light_glyph(&logo).then_some([58, 58, 60]))
    };
    let finished = match tile {
        Some(color) => plate(&logo, color),
        None => logo,
    };

    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(finished)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    ))
}

/// Install folder of the newest installed package in `family`.
fn package_root_for_family(family: &str) -> Option<PathBuf> {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Storage::Packaging::Appx::{
        GetPackagePathByFullName, GetPackagesByPackageFamily,
    };

    let family_w: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut count = 0u32;
        let mut buffer_len = 0u32;
        let first = GetPackagesByPackageFamily(
            PCWSTR(family_w.as_ptr()),
            &mut count,
            None,
            &mut buffer_len,
            None,
        );
        if first.0 != ERROR_INSUFFICIENT_BUFFER || count == 0 {
            return None;
        }
        let mut names = vec![PWSTR::null(); count as usize];
        let mut buffer = vec![0u16; buffer_len as usize];
        let second = GetPackagesByPackageFamily(
            PCWSTR(family_w.as_ptr()),
            &mut count,
            Some(names.as_mut_ptr()),
            &mut buffer_len,
            Some(PWSTR(buffer.as_mut_ptr())),
        );
        if second.0 != 0 {
            return None;
        }
        // Several versions can be staged during an update; the last is newest.
        for name in names.iter().take(count as usize).rev() {
            let mut path_len = 0u32;
            let _ = GetPackagePathByFullName(PCWSTR(name.0), &mut path_len, None);
            if path_len == 0 {
                continue;
            }
            let mut path = vec![0u16; path_len as usize];
            if GetPackagePathByFullName(
                PCWSTR(name.0),
                &mut path_len,
                Some(PWSTR(path.as_mut_ptr())),
            )
            .0 == 0
            {
                let end = path.iter().position(|&c| c == 0).unwrap_or(path.len());
                return Some(PathBuf::from(String::from_utf16_lossy(&path[..end])));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10">
  <Applications>
    <Application Id="Helper" Executable="helper\Helper.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="Helper" Square150x150Logo="Assets\Helper150.png" Square44x44Logo="Assets\Helper44.png" BackgroundColor="transparent" />
    </Application>
    <Application Id="App" Executable="WhatsApp.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="WhatsApp" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" BackgroundColor="#25D366">
        <uap:DefaultTile Wide310x150Logo="Assets\Wide.png" />
      </uap:VisualElements>
    </Application>
  </Applications>
</Package>"##;

    #[test]
    fn reads_visual_elements_by_app_id() {
        let v = manifest_visuals(MANIFEST, &AppSelector::Id("App")).unwrap();
        assert_eq!(v.square44.as_deref(), Some("Assets\\Square44x44Logo.png"));
        assert_eq!(
            v.square150.as_deref(),
            Some("Assets\\Square150x150Logo.png")
        );
        assert_eq!(v.background, Some([0x25, 0xD3, 0x66]));
        assert!(manifest_visuals(MANIFEST, &AppSelector::Id("Missing")).is_none());
    }

    #[test]
    fn reads_visual_elements_by_executable() {
        let v = manifest_visuals(MANIFEST, &AppSelector::Executable("helper\\helper.exe")).unwrap();
        assert_eq!(v.square44.as_deref(), Some("Assets\\Helper44.png"));
        assert_eq!(v.background, None);
        // Unknown exe falls back to the first application.
        let v = manifest_visuals(MANIFEST, &AppSelector::Executable("Other.exe")).unwrap();
        assert_eq!(v.square44.as_deref(), Some("Assets\\Helper44.png"));
    }

    #[test]
    fn scores_qualified_assets() {
        assert_eq!(
            score_asset("Square44x44Logo.targetsize-256_altform-unplated.png", 44),
            Some((true, 256))
        );
        assert_eq!(
            score_asset("Square44x44Logo.scale-200.png", 44),
            Some((false, 88))
        );
        assert_eq!(
            score_asset("scale-400\\Square150x150Logo.png", 150),
            Some((false, 256))
        );
        assert_eq!(
            score_asset(
                "Square44x44Logo.targetsize-48_altform-lightunplated.png",
                44
            ),
            Some((false, 48))
        );
        assert_eq!(
            score_asset("Square44x44Logo.contrast-black_scale-200.png", 44),
            None
        );
    }

    #[test]
    fn picks_best_file_on_disk() {
        let dir = std::env::temp_dir().join(format!("orbix-pkg-icon-{}", std::process::id()));
        let assets = dir.join("Assets");
        std::fs::create_dir_all(&assets).unwrap();
        for name in [
            "Square44x44Logo.scale-100.png",
            "Square44x44Logo.targetsize-48.png",
            "Square44x44Logo.targetsize-256_altform-unplated.png",
            "Square44x44Logo.contrast-white_targetsize-256.png",
            "Other.targetsize-256_altform-unplated.png",
        ] {
            std::fs::write(assets.join(name), b"x").unwrap();
        }
        let (path, unplated, size) = best_asset(&dir, "Assets\\Square44x44Logo.png", 44).unwrap();
        assert!(path.ends_with("Square44x44Logo.targetsize-256_altform-unplated.png"));
        assert!(unplated);
        assert_eq!(size, 256);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plates_white_glyphs_only() {
        let mut white = image::RgbaImage::new(ICON_SIZE, ICON_SIZE);
        white.put_pixel(64, 64, image::Rgba([255, 255, 255, 255]));
        assert!(is_light_glyph(&white));
        let mut green = image::RgbaImage::new(ICON_SIZE, ICON_SIZE);
        green.put_pixel(64, 64, image::Rgba([30, 215, 96, 255]));
        assert!(!is_light_glyph(&green));

        let tiled = plate(&white, [37, 211, 102]);
        assert_eq!(tiled.get_pixel(64, 64).0, [255, 255, 255, 255]);
        assert_eq!(tiled.get_pixel(10, 64).0, [37, 211, 102, 255]);
        assert_eq!(tiled.get_pixel(0, 0)[3], 0, "corners are rounded off");
    }

    #[test]
    fn parses_colors() {
        assert_eq!(parse_color("#1DB954"), Some([0x1D, 0xB9, 0x54]));
        assert_eq!(parse_color("transparent"), None);
    }
}
