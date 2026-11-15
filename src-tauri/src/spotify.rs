//! "Like" the playing song in Spotify from the notch.
//!
//! Spotify only allows this through its Web API, which needs an app the user
//! registers at developer.spotify.com (its Client ID is pasted in Settings).
//! Sign-in uses the Authorization Code flow with PKCE: no client secret, the
//! browser hands the code back to a one-shot listener on 127.0.0.1. Tokens are
//! encrypted with Windows DPAPI (readable only by this Windows user) before
//! they touch the disk.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub const REDIRECT_PORT: u16 = 43821;
const REDIRECT_URI: &str = "http://127.0.0.1:43821/callback";
const SCOPES: &str =
    "user-library-read user-library-modify user-read-currently-playing user-read-playback-state";
const TOKEN_FILE: &str = "spotify_token.bin";
const LOGIN_TIMEOUT_SECS: u64 = 180;

#[derive(Serialize, Deserialize, Clone)]
struct Tokens {
    client_id: String,
    access_token: String,
    refresh_token: String,
    /// Unix seconds.
    expires_at: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct TrackState {
    pub track_id: String,
    pub liked: bool,
}

static TOKENS: Mutex<Option<Tokens>> = Mutex::new(None);

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// PKCE verifier and its S256 challenge.
fn pkce_pair() -> Result<(String, String), String> {
    use ring::rand::SecureRandom;
    let mut random = [0u8; 48];
    ring::rand::SystemRandom::new()
        .fill(&mut random)
        .map_err(|_| "No secure random source")?;
    let verifier = b64url(&random);
    let challenge =
        b64url(ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes()).as_ref());
    Ok((verifier, challenge))
}

fn random_state() -> Result<String, String> {
    use ring::rand::SecureRandom;
    let mut random = [0u8; 16];
    ring::rand::SystemRandom::new()
        .fill(&mut random)
        .map_err(|_| "No secure random source")?;
    Ok(b64url(&random))
}

// ── Encrypted token storage (DPAPI) ─────────────────────────────────────────

fn dpapi(data: &[u8], protect: bool) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        if protect {
            CryptProtectData(
                &input,
                windows::core::PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    }
    .is_ok();
    if !ok || output.pbData.is_null() {
        return None;
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
    }
    Some(bytes)
}

fn token_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join(TOKEN_FILE))
}

fn save_tokens(app: &AppHandle, tokens: &Tokens) -> Result<(), String> {
    let path = token_path(app).ok_or("No config folder")?;
    let json = serde_json::to_vec(tokens).map_err(|e| e.to_string())?;
    let sealed = dpapi(&json, true).ok_or("Could not encrypt the Spotify sign-in")?;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(path, sealed).map_err(|e| e.to_string())?;
    if let Ok(mut slot) = TOKENS.lock() {
        *slot = Some(tokens.clone());
    }
    Ok(())
}

fn load_tokens(app: &AppHandle) -> Option<Tokens> {
    if let Some(t) = TOKENS.lock().ok().and_then(|t| t.clone()) {
        return Some(t);
    }
    let sealed = std::fs::read(token_path(app)?).ok()?;
    let tokens: Tokens = serde_json::from_slice(&dpapi(&sealed, false)?).ok()?;
    if let Ok(mut slot) = TOKENS.lock() {
        *slot = Some(tokens.clone());
    }
    Some(tokens)
}

// ── HTTP ────────────────────────────────────────────────────────────────────

fn client() -> Result<reqwest::Client, String> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())
}

fn form_body(params: &[(&str, &str)]) -> String {
    reqwest::Url::parse_with_params("http://x/", params)
        .ok()
        .and_then(|u| u.query().map(str::to_string))
        .unwrap_or_default()
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

async fn token_request(params: &[(&str, &str)]) -> Result<TokenResponse, String> {
    let response = client()?
        .post("https://accounts.spotify.com/api/token")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_body(params))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Spotify sign-in failed ({status}): {body}"));
    }
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

/// A valid access token, refreshing it when it is about to expire.
async fn access_token(app: &AppHandle) -> Result<String, String> {
    let mut tokens = load_tokens(app).ok_or("NOT_CONNECTED")?;
    if tokens.expires_at > now_secs() + 60 {
        return Ok(tokens.access_token);
    }
    let refreshed = token_request(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", &tokens.refresh_token),
        ("client_id", &tokens.client_id),
    ])
    .await?;
    tokens.access_token = refreshed.access_token;
    if let Some(r) = refreshed.refresh_token {
        tokens.refresh_token = r;
    }
    tokens.expires_at = now_secs() + refreshed.expires_in;
    save_tokens(app, &tokens)?;
    Ok(tokens.access_token)
}

async fn api(
    app: &AppHandle,
    method: reqwest::Method,
    path: &str,
    params: &[(&str, &str)],
) -> Result<(reqwest::StatusCode, String), String> {
    let token = access_token(app).await?;
    let url = reqwest::Url::parse_with_params(&format!("https://api.spotify.com/v1{path}"), params)
        .map_err(|e| e.to_string())?;
    let response = client()?
        .request(method, url)
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err("Spotify refused the request (the developer app's owner needs Spotify Premium, and your account must be added to the app's users)".into());
    }
    Ok((status, body))
}

// ── Sign-in ─────────────────────────────────────────────────────────────────

/// Waits for the browser's redirect and returns its query string.
fn wait_for_redirect(listener: std::net::TcpListener) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(LOGIN_TIMEOUT_SECS);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);
                let target = request.split_whitespace().nth(1).unwrap_or("").to_string();
                if !target.starts_with("/callback") {
                    let _ =
                        stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                    continue;
                }
                let page = "<html><body style=\"font-family:system-ui;background:#111;color:#eee;display:grid;place-items:center;height:100vh\"><h2>Orbix is connected to Spotify. You can close this tab.</h2></body></html>";
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
                        page.len()
                    )
                    .as_bytes(),
                );
                return Ok(target
                    .split_once('?')
                    .map(|(_, q)| q.to_string())
                    .unwrap_or_default());
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() > deadline {
                    return Err("Sign-in timed out".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

fn open_in_browser(url: &str) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        ShellExecuteW(
            None,
            windows::core::PCWSTR(verb.as_ptr()),
            windows::core::PCWSTR(wide.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

fn query_value(query: &str, key: &str) -> Option<String> {
    reqwest::Url::parse(&format!("http://x/?{query}"))
        .ok()?
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.to_string())
}

#[tauri::command]
pub async fn spotify_connect(app: AppHandle, client_id: String) -> Result<(), String> {
    let client_id = client_id.trim().to_string();
    if client_id.len() < 16 || !client_id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("That doesn't look like a Spotify Client ID".into());
    }
    let (verifier, challenge) = pkce_pair()?;
    let state = random_state()?;
    let listener = std::net::TcpListener::bind(("127.0.0.1", REDIRECT_PORT))
        .map_err(|_| format!("Port {REDIRECT_PORT} is busy; close the other sign-in and retry"))?;
    let authorize = reqwest::Url::parse_with_params(
        "https://accounts.spotify.com/authorize",
        &[
            ("client_id", client_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", REDIRECT_URI),
            ("code_challenge_method", "S256"),
            ("code_challenge", challenge.as_str()),
            ("scope", SCOPES),
            ("state", state.as_str()),
        ],
    )
    .map_err(|e| e.to_string())?;
    open_in_browser(authorize.as_str());

    let query = tauri::async_runtime::spawn_blocking(move || wait_for_redirect(listener))
        .await
        .map_err(|e| e.to_string())??;
    if query_value(&query, "state").as_deref() != Some(state.as_str()) {
        return Err("Sign-in response didn't match this request".into());
    }
    if let Some(error) = query_value(&query, "error") {
        return Err(format!("Spotify sign-in was cancelled ({error})"));
    }
    let code = query_value(&query, "code").ok_or("Spotify didn't return a sign-in code")?;
    let tokens = token_request(&[
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", REDIRECT_URI),
        ("client_id", &client_id),
        ("code_verifier", &verifier),
    ])
    .await?;
    save_tokens(
        &app,
        &Tokens {
            client_id,
            access_token: tokens.access_token,
            refresh_token: tokens
                .refresh_token
                .ok_or("Spotify didn't return a refresh token")?,
            expires_at: now_secs() + tokens.expires_in,
        },
    )
}

#[tauri::command]
pub fn spotify_status(app: AppHandle) -> bool {
    load_tokens(&app).is_some()
}

#[tauri::command]
pub fn spotify_disconnect(app: AppHandle) {
    if let Ok(mut slot) = TOKENS.lock() {
        *slot = None;
    }
    if let Some(path) = token_path(&app) {
        let _ = std::fs::remove_file(path);
    }
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// True when the Spotify item is the song the notch is showing.
fn same_song(spotify_name: &str, notch_title: &str) -> bool {
    let a = normalize(spotify_name);
    let b = normalize(notch_title);
    !a.is_empty() && !b.is_empty() && (a == b || a.contains(&b) || b.contains(&a))
}

/// The playing Spotify track (if it is `title`) and whether it is liked.
#[tauri::command]
pub async fn spotify_track_state(
    app: AppHandle,
    title: String,
) -> Result<Option<TrackState>, String> {
    let (status, body) = api(
        &app,
        reqwest::Method::GET,
        "/me/player/currently-playing",
        &[],
    )
    .await?;
    if status == reqwest::StatusCode::NO_CONTENT || body.is_empty() {
        return Ok(None);
    }
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let item = &json["item"];
    let (Some(id), Some(name)) = (item["id"].as_str(), item["name"].as_str()) else {
        return Ok(None); // ads, local files, podcasts
    };
    if item["type"].as_str() != Some("track") || !same_song(name, &title) {
        return Ok(None);
    }
    let uri = format!("spotify:track:{id}");
    let (_, contains) = api(
        &app,
        reqwest::Method::GET,
        "/me/library/contains",
        &[("uris", &uri)],
    )
    .await?;
    let liked = serde_json::from_str::<Vec<bool>>(&contains)
        .ok()
        .and_then(|v| v.first().copied())
        .unwrap_or(false);
    Ok(Some(TrackState {
        track_id: id.to_string(),
        liked,
    }))
}

#[tauri::command]
pub async fn spotify_set_liked(
    app: AppHandle,
    track_id: String,
    liked: bool,
) -> Result<bool, String> {
    if !track_id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Invalid track".into());
    }
    let uri = format!("spotify:track:{track_id}");
    let method = if liked {
        reqwest::Method::PUT
    } else {
        reqwest::Method::DELETE
    };
    let (status, body) = api(&app, method, "/me/library", &[("uris", &uri)]).await?;
    if !status.is_success() {
        return Err(format!("Spotify returned {status}: {body}"));
    }
    Ok(liked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_song_titles_loosely() {
        assert!(same_song(
            "Ma Meilleure Ennemie",
            "Ma Meilleure Ennemie (from Arcane)"
        ));
        assert!(same_song(
            "Don't Stop Me Now - Remastered 2011",
            "Don’t Stop Me Now"
        ));
        assert!(!same_song("Yesterday", "Let It Be"));
        assert!(!same_song("", "x"));
    }

    #[test]
    fn builds_form_bodies() {
        assert_eq!(form_body(&[("a", "x y"), ("b", "c&d")]), "a=x+y&b=c%26d");
    }

    #[test]
    fn reads_query_values() {
        assert_eq!(
            query_value("code=abc&state=xyz", "state").as_deref(),
            Some("xyz")
        );
        assert_eq!(query_value("error=access_denied", "code"), None);
    }
}
