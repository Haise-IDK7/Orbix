//! Lyrics translation for the notch's lyrics panel (cloud engines).
//!
//! The panel first tries the translator built into WebView2/Edge, which runs
//! on this PC with no limit; this module is the fallback when that isn't
//! available:
//! 1. Microsoft Translator with the user's own free key (Azure F0: 2 million
//!    characters a month, no daily cap), kept encrypted with DPAPI.
//! 2. MyMemory, no key (about 5,000 characters a day; 50,000 with an email).
//!
//! Either way, repeated lines (choruses) are sent once and each song's
//! translation is saved to disk, so replays cost nothing.

use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tauri::{AppHandle, Manager};

const ENDPOINT: &str = "https://api.mymemory.translated.net/get";
/// MyMemory rejects queries over 500 bytes.
const MAX_CHUNK_BYTES: usize = 450;

#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Translation {
    /// One entry per input line ("" for blank lines).
    pub lines: Vec<String>,
    /// Detected language of the lyrics, e.g. "en". Equal to the target when
    /// the song is already in that language (then `lines` are the originals).
    pub source: String,
    /// "microsoft" or "mymemory".
    #[serde(default)]
    pub engine: String,
}

const MICROSOFT_ENDPOINT: &str = "https://api.cognitive.microsofttranslator.com/translate";
const KEY_FILE: &str = "translator_key.bin";

#[derive(Serialize, serde::Deserialize)]
struct MicrosoftKey {
    key: String,
    /// Azure region of the resource ("eastus"), or "" for a global resource.
    region: String,
}

fn key_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join(KEY_FILE))
}

fn load_key(app: &AppHandle) -> Option<MicrosoftKey> {
    let bytes = std::fs::read(key_path(app)?).ok()?;
    let plain = crate::spotify::dpapi(&bytes, false)?;
    serde_json::from_slice(&plain).ok()
}

/// Sends all lines in one request (the API takes up to 1,000 texts) and gets
/// one translation per line back, plus the detected language of each.
async fn microsoft(
    lines: &[String],
    target: &str,
    key: &MicrosoftKey,
) -> Result<(Vec<String>, Option<String>), String> {
    let client = crate::lyrics::client()?;
    let url = reqwest::Url::parse_with_params(
        MICROSOFT_ENDPOINT,
        &[("api-version", "3.0"), ("to", target)],
    )
    .map_err(|e| e.to_string())?;
    let body: Vec<serde_json::Value> = lines
        .iter()
        .map(|l| serde_json::json!({ "Text": l }))
        .collect();
    let mut request = client
        .post(url)
        .header("Ocp-Apim-Subscription-Key", &key.key)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).map_err(|e| e.to_string())?);
    if !key.region.is_empty() {
        request = request.header("Ocp-Apim-Subscription-Region", &key.region);
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err("Microsoft Translator didn't accept the key or region".into());
    }
    if status.as_u16() == 429 {
        return Err("Microsoft Translator's free monthly characters are used up".into());
    }
    if !status.is_success() {
        return Err(format!("Microsoft Translator returned {status}"));
    }
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let items = json
        .as_array()
        .ok_or("Unexpected Microsoft Translator reply")?;
    if items.len() != lines.len() {
        return Err("Unexpected Microsoft Translator reply".into());
    }
    let mut votes: Vec<(String, usize)> = Vec::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(
            item["translations"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .trim()
                .to_string(),
        );
        if let Some(lang) = item["detectedLanguage"]["language"].as_str() {
            let lang: String = lang.chars().take(2).collect::<String>().to_lowercase();
            match votes.iter_mut().find(|(l, _)| *l == lang) {
                Some((_, n)) => *n += 1,
                None => votes.push((lang, 1)),
            }
        }
    }
    let detected = votes.into_iter().max_by_key(|(_, n)| *n).map(|(l, _)| l);
    Ok((out, detected))
}

/// Groups lines into newline-joined batches under the request size limit.
fn chunks(lines: &[String]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut size = 0;
    for line in lines {
        let line = line.chars().take(MAX_CHUNK_BYTES / 4).collect::<String>();
        let len = line.len() + 1;
        match out.last_mut() {
            Some(chunk) if size + len <= MAX_CHUNK_BYTES => {
                chunk.push(line);
                size += len;
            }
            _ => {
                out.push(vec![line]);
                size = len;
            }
        }
    }
    out
}

/// MyMemory returns HTML entities (`&#39;`, `&quot;`) in its text.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find(';').filter(|&e| e <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

struct Reply {
    text: String,
    detected: Option<String>,
}

async fn request(
    text: &str,
    source: &str,
    target: &str,
    email: Option<&str>,
) -> Result<Reply, String> {
    let client = crate::lyrics::client()?;
    let langpair = format!("{source}|{target}");
    let mut query: Vec<(&str, &str)> = vec![("q", text), ("langpair", &langpair)];
    if let Some(email) = email {
        query.push(("de", email));
    }
    let url = reqwest::Url::parse_with_params(ENDPOINT, &query).map_err(|e| e.to_string())?;
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    let http = response.status();
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let status = json["responseStatus"]
        .as_i64()
        .or_else(|| json["responseStatus"].as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(http.as_u16() as i64);
    if status == 429 || json["quotaFinished"].as_bool() == Some(true) {
        return Err(if email.is_some() {
            "Today's free translation limit is used up. It resets tomorrow.".into()
        } else {
            "Today's free translation limit is used up. Add an email in Settings → Notch to raise it.".into()
        });
    }
    if status != 200 {
        let detail = json["responseDetails"]
            .as_str()
            .unwrap_or("translation failed");
        return Err(format!("Translator: {detail}"));
    }
    Ok(Reply {
        text: unescape(
            json["responseData"]["translatedText"]
                .as_str()
                .unwrap_or_default(),
        ),
        detected: json["responseData"]["detectedLanguage"]
            .as_str()
            .map(|s| s.chars().take(2).collect::<String>().to_lowercase()),
    })
}

/// Translates one batch, keeping one output line per input line. Falls back
/// to line-by-line if the service merged or split lines.
async fn translate_chunk(
    chunk: &[String],
    source: &str,
    target: &str,
    email: Option<&str>,
) -> Result<(Vec<String>, Option<String>), String> {
    let reply = request(&chunk.join("\n"), source, target, email).await?;
    let parts: Vec<String> = reply
        .text
        .split('\n')
        .map(|s| s.trim().to_string())
        .collect();
    if parts.len() == chunk.len() {
        return Ok((parts, reply.detected));
    }
    let mut out = Vec::with_capacity(chunk.len());
    for line in chunk {
        out.push(
            request(line, source, target, email)
                .await?
                .text
                .trim()
                .to_string(),
        );
    }
    Ok((out, reply.detected))
}

fn cache_path(app: &AppHandle, lines: &[String], target: &str) -> Option<std::path::PathBuf> {
    let mut hasher = DefaultHasher::new();
    lines.hash(&mut hasher);
    target.hash(&mut hasher);
    let dir = app.path().app_cache_dir().ok()?.join("translations");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("{:016x}.json", hasher.finish())))
}

/// MyMemory, batch by batch. Returns the translations and the detected
/// source language ("" if unknown); stops early (empty list) when the song is
/// already in `target`.
async fn mymemory(
    unique: &[String],
    target: &str,
    email: Option<&str>,
) -> Result<(Vec<String>, String), String> {
    let mut source = "autodetect".to_string();
    let mut translated: Vec<String> = Vec::with_capacity(unique.len());
    for chunk in chunks(unique) {
        let (out, detected) = translate_chunk(&chunk, &source, target, email).await?;
        if source == "autodetect" {
            if let Some(lang) = detected {
                if lang == target {
                    // Already in the target language: nothing to spend.
                    return Ok((Vec::new(), lang));
                }
                source = lang;
            }
        }
        translated.extend(out);
    }

    if source == "autodetect" {
        source.clear();
    }
    Ok((translated, source))
}

/// Whether a Microsoft Translator key is saved; returns its region.
#[tauri::command]
pub fn translator_key_status(app: AppHandle) -> Option<String> {
    load_key(&app).map(|k| k.region)
}

/// Checks the key with a one-word translation, then saves it encrypted.
#[tauri::command]
pub async fn set_translator_key(app: AppHandle, key: String, region: String) -> Result<(), String> {
    let key = MicrosoftKey {
        key: key.trim().to_string(),
        region: region.trim().to_lowercase(),
    };
    if key.key.len() < 16 || !key.key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("That doesn't look like a Translator key".into());
    }
    if !key.region.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Region should look like \"eastus\" or \"global\"".into());
    }
    microsoft(&["hello".to_string()], "es", &key).await?;
    let path = key_path(&app).ok_or("No config folder")?;
    let plain = serde_json::to_vec(&key).map_err(|e| e.to_string())?;
    let sealed = crate::spotify::dpapi(&plain, true).ok_or("Couldn't encrypt the key")?;
    std::fs::write(path, sealed).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_translator_key(app: AppHandle) {
    if let Some(path) = key_path(&app) {
        let _ = std::fs::remove_file(path);
    }
}

#[tauri::command]
pub async fn translate_lyrics(
    app: AppHandle,
    lines: Vec<String>,
    target: String,
) -> Result<Translation, String> {
    let target = target.to_lowercase();
    if target.len() != 2 || !target.chars().all(|c| c.is_ascii_lowercase()) {
        return Err("Unsupported language".into());
    }
    let path = cache_path(&app, &lines, &target);
    if let Some(hit) = path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Translation>(&s).ok())
    {
        return Ok(hit);
    }

    let email = crate::utils::get_setting_str(&app, "bloom-translate-email")
        .map(|e| e.trim().to_string())
        .filter(|e| e.contains('@') && e.len() <= 120);

    let mut unique: Vec<String> = Vec::new();
    for line in &lines {
        let line = line.trim();
        if !line.is_empty() && !unique.iter().any(|u| u == line) {
            unique.push(line.to_string());
        }
    }

    let (translated, source, engine) = match load_key(&app) {
        Some(key) => match microsoft(&unique, &target, &key).await {
            Ok((out, detected)) => (out, detected.unwrap_or_default(), "microsoft"),
            // Key problems or an empty month: keep translating with MyMemory.
            Err(_) => {
                let (out, source) = mymemory(&unique, &target, email.as_deref()).await?;
                (out, source, "mymemory")
            }
        },
        None => {
            let (out, source) = mymemory(&unique, &target, email.as_deref()).await?;
            (out, source, "mymemory")
        }
    };
    if source == target {
        // Already in the target language: show the originals.
        return Ok(Translation {
            lines,
            source,
            engine: engine.into(),
        });
    }

    let result = Translation {
        lines: lines
            .iter()
            .map(|line| {
                let line = line.trim();
                unique
                    .iter()
                    .position(|u| u == line)
                    .and_then(|i| translated.get(i).cloned())
                    .unwrap_or_default()
            })
            .collect(),
        source,
        engine: engine.into(),
    };
    if let (Some(path), Ok(json)) = (path, serde_json::to_string(&result)) {
        let _ = std::fs::write(path, json);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_entities() {
        assert_eq!(
            unescape("No llores &#39;esta noche&#39; &amp; m&#xE1;s"),
            "No llores 'esta noche' & más"
        );
        assert_eq!(unescape("Tom & Jerry; &unknown;"), "Tom & Jerry; &unknown;");
        assert_eq!(unescape("&quot;hola&quot;"), "\"hola\"");
    }

    #[test]
    fn chunks_respect_size_limit() {
        let lines: Vec<String> = (0..40)
            .map(|i| format!("This is lyric line number {i}"))
            .collect();
        let batches = chunks(&lines);
        assert!(batches.len() > 1);
        for batch in &batches {
            assert!(batch.join("\n").len() <= MAX_CHUNK_BYTES);
        }
        assert_eq!(batches.concat(), lines);
    }
}
