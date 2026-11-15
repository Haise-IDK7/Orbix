//! Lyrics translation for the notch's lyrics panel.
//!
//! Uses MyMemory's free API (no key). Anonymous use allows about 5,000
//! characters a day; an email in Settings raises that to 50,000. To spend as
//! little as possible, repeated lines (choruses) are sent once, lines go in
//! batches, and each song's translation is saved to disk so replays are free.

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

    let mut source = "autodetect".to_string();
    let mut translated: Vec<String> = Vec::with_capacity(unique.len());
    for chunk in chunks(&unique) {
        let (out, detected) = translate_chunk(&chunk, &source, &target, email.as_deref()).await?;
        if source == "autodetect" {
            if let Some(lang) = detected {
                if lang == target {
                    // Already in the target language: nothing to spend.
                    return Ok(Translation {
                        lines,
                        source: lang,
                    });
                }
                source = lang;
            }
        }
        translated.extend(out);
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
        source: if source == "autodetect" {
            String::new()
        } else {
            source
        },
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
