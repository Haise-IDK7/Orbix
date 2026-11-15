//! Time-synced lyrics from LRCLIB (https://lrclib.net), a free, keyless,
//! community lyrics database. The notch's lyrics panel calls `fetch_lyrics`
//! once per track; results (including misses) are cached for the session.
//!
//! Fetched in Rust rather than the webview so no CSP/CORS exception is needed.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const API_BASE: &str = "https://lrclib.net/api/search";
const USER_AGENT: &str = "Orbix (https://github.com/Lukog10/Orbix)";
/// Candidates whose length differs more than this are a different recording.
const MAX_DURATION_DIFF_SECS: f64 = 8.0;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LyricsResult {
    /// LRC text with `[mm:ss.xx]` timestamps, when available.
    pub synced: Option<String>,
    pub plain: Option<String>,
    pub instrumental: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
}

const MAX_CACHED_SONGS: usize = 150;

fn cache() -> &'static Mutex<HashMap<String, Option<LyricsResult>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<LyricsResult>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c);
    }
    // reqwest is built with `rustls-no-provider` (via the updater plugin), so a
    // crypto provider must be installed before the first HTTPS client exists.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let built = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(CLIENT.get_or_init(|| built))
}

/// Strips decorations players add to titles: "(feat. X)", "- Remastered 2011",
/// "[Official Video]", "(Live)" etc. LRCLIB indexes the plain song name.
fn clean_title(title: &str) -> String {
    let mut t = title.to_string();
    // Bracketed suffixes.
    loop {
        let trimmed = t.trim_end();
        let Some(close) = trimmed.chars().last().filter(|c| *c == ')' || *c == ']') else {
            break;
        };
        let open = if close == ')' { '(' } else { '[' };
        match trimmed.rfind(open) {
            Some(idx) if idx > 0 => t = trimmed[..idx].to_string(),
            _ => break,
        }
    }
    // " - Remastered 2009", " - Radio Edit", " - Live at ..." suffixes.
    const DASH_SUFFIXES: &[&str] = &[
        "remaster",
        "remastered",
        "radio edit",
        "live",
        "mono",
        "stereo",
        "single version",
        "edit",
        "version",
        "acoustic",
        "bonus track",
        "explicit",
        "clean",
    ];
    if let Some(idx) = t.rfind(" - ") {
        let suffix = t[idx + 3..].to_lowercase();
        if DASH_SUFFIXES.iter().any(|s| suffix.contains(s)) {
            t.truncate(idx);
        }
    }
    let lower = t.to_lowercase();
    for marker in [" feat. ", " ft. ", " featuring "] {
        if let Some(idx) = lower.find(marker) {
            t.truncate(idx);
            break;
        }
    }
    t.trim().to_string()
}

/// "Artist A, Artist B" / "A & B" / "A feat. B" → "Artist A".
fn primary_artist(artist: &str) -> String {
    let lower = artist.to_lowercase();
    let mut cut = artist.len();
    for sep in [", ", " & ", " feat. ", " ft. ", " x ", "; ", " / "] {
        if let Some(idx) = lower.find(sep) {
            cut = cut.min(idx);
        }
    }
    artist[..cut].trim().to_string()
}

/// Search attempts from most to least specific, without duplicates.
fn search_attempts(title: &str, artist: &str) -> Vec<(String, String)> {
    let mut attempts: Vec<(String, String)> = Vec::new();
    let mut push = |t: String, a: String| {
        if !t.is_empty() && !attempts.iter().any(|(x, y)| *x == t && *y == a) {
            attempts.push((t, a));
        }
    };
    push(title.trim().to_string(), artist.trim().to_string());
    push(clean_title(title), primary_artist(artist));
    // Browser players (YouTube) often send "Artist - Song" as the title and the
    // channel as the artist.
    if let Some((left, right)) = title.split_once(" - ") {
        push(clean_title(right), primary_artist(left));
    }
    push(clean_title(title), String::new());
    attempts
}

/// Best match: synced lyrics first, then closest duration.
fn pick_best(tracks: &[LrclibTrack], duration_secs: Option<f64>) -> Option<LrclibTrack> {
    let diff = |t: &LrclibTrack| match (duration_secs, t.duration) {
        (Some(want), Some(have)) if want > 0.0 && have > 0.0 => (want - have).abs(),
        _ => 0.0,
    };
    let has_lyrics = |t: &LrclibTrack| {
        t.instrumental
            || t.synced_lyrics
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
            || t.plain_lyrics
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
    };
    tracks
        .iter()
        .filter(|t| has_lyrics(t) && diff(t) <= MAX_DURATION_DIFF_SECS)
        .min_by(|a, b| {
            let synced_a = a.synced_lyrics.is_some();
            let synced_b = b.synced_lyrics.is_some();
            synced_b.cmp(&synced_a).then(
                diff(a)
                    .partial_cmp(&diff(b))
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        })
        .cloned()
}

async fn search(track: &str, artist: &str) -> Result<Vec<LrclibTrack>, String> {
    let mut params = vec![("track_name", track)];
    if !artist.is_empty() {
        params.push(("artist_name", artist));
    }
    let url = reqwest::Url::parse_with_params(API_BASE, &params).map_err(|e| e.to_string())?;
    let response = client()?.get(url).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("LRCLIB returned HTTP {}", response.status()));
    }
    let body = response.text().await.map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_lyrics(
    title: String,
    artist: String,
    duration_ms: i64,
) -> Result<Option<LyricsResult>, String> {
    if title.trim().is_empty() {
        return Ok(None);
    }
    let key = format!("{}\u{1f}{}", artist.to_lowercase(), title.to_lowercase());
    if let Ok(guard) = cache().lock() {
        if let Some(hit) = guard.get(&key) {
            return Ok(hit.clone());
        }
    }

    let duration_secs = (duration_ms > 0).then(|| duration_ms as f64 / 1000.0);
    let mut found = None;
    let mut last_error = None;
    for (track, artist_query) in search_attempts(&title, &artist) {
        match search(&track, &artist_query).await {
            Ok(tracks) => {
                if let Some(best) = pick_best(&tracks, duration_secs) {
                    found = Some(LyricsResult {
                        synced: best.synced_lyrics.filter(|s| !s.trim().is_empty()),
                        plain: best.plain_lyrics.filter(|s| !s.trim().is_empty()),
                        instrumental: best.instrumental,
                    });
                    break;
                }
            }
            Err(e) => last_error = Some(e),
        }
    }

    // Don't cache network failures, so the next play retries.
    if found.is_none() {
        if let Some(e) = last_error {
            return Err(e);
        }
    }
    if let Ok(mut guard) = cache().lock() {
        // A session rarely plays this many songs; start over rather than grow.
        if guard.len() >= MAX_CACHED_SONGS {
            guard.clear();
        }
        guard.insert(key, found.clone());
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_common_title_decorations() {
        assert_eq!(clean_title("Yesterday - Remastered 2009"), "Yesterday");
        assert_eq!(clean_title("Song (feat. Someone) [Official Video]"), "Song");
        assert_eq!(clean_title("Song feat. Someone"), "Song");
        assert_eq!(clean_title("Hold On - Part 2"), "Hold On - Part 2");
        assert_eq!(
            clean_title("(What's The Story) Morning Glory?"),
            "(What's The Story) Morning Glory?"
        );
    }

    #[test]
    fn extracts_primary_artist() {
        assert_eq!(primary_artist("Daft Punk, Pharrell Williams"), "Daft Punk");
        assert_eq!(primary_artist("Simon & Garfunkel"), "Simon");
        assert_eq!(primary_artist("Adele"), "Adele");
    }

    #[test]
    fn attempts_include_youtube_style_split() {
        let attempts = search_attempts("Adele - Hello (Official Video)", "AdeleVEVO");
        assert!(attempts.contains(&("Hello".to_string(), "Adele".to_string())));
    }

    #[test]
    fn prefers_synced_and_rejects_wrong_duration() {
        let plain = LrclibTrack {
            duration: Some(200.0),
            instrumental: false,
            plain_lyrics: Some("a".into()),
            synced_lyrics: None,
        };
        let synced = LrclibTrack {
            synced_lyrics: Some("[00:01.00]a".into()),
            ..plain.clone()
        };
        let wrong_length = LrclibTrack {
            duration: Some(400.0),
            ..synced.clone()
        };
        let best = pick_best(&[plain.clone(), wrong_length, synced], Some(201.0)).unwrap();
        assert!(best.synced_lyrics.is_some());
        assert!(pick_best(&[plain], Some(300.0)).is_none());
    }
}
