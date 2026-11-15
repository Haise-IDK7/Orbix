//! Native Wi-Fi support for the notch quick settings.
//!
//! Everything goes through the WLAN API (wlanapi.dll) instead of parsing
//! `netsh wlan` output. `netsh` text is localized (e.g. "Señal" /
//! "Autenticación" on Spanish Windows), which silently broke signal and
//! security detection, and `show networks` only returns a stale cache.
//!
//! Windows 11 24H2+ treats Wi-Fi scan results and SSIDs as location data: if
//! "Let desktop apps access your location" is off, the scan list and the
//! current SSID come back as ERROR_ACCESS_DENIED. Association state from
//! `WlanEnumInterfaces` is still readable, so "connected" never depends on it.

use crate::types::WifiNetworkEntry;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WiFi::*;

/// Error string the frontend matches to show the location-permission hint.
pub const LOCATION_PERMISSION_ERROR: &str = "LOCATION_PERMISSION_REQUIRED";

const ERROR_ACCESS_DENIED: u32 = 5;
const SCAN_SETTLE_MS: u64 = 2500;
const CONNECT_TIMEOUT_MS: u64 = 15_000;

struct WlanClient(HANDLE);

impl WlanClient {
    fn open() -> Result<Self, String> {
        let mut negotiated_version = 0u32;
        let mut handle = HANDLE::default();
        let ret = unsafe { WlanOpenHandle(2, None, &mut negotiated_version, &mut handle) };
        if ret != 0 {
            return Err(format!("WLAN service unavailable (error {ret})"));
        }
        Ok(Self(handle))
    }

    /// Copies the interface list out of WLAN-owned memory.
    fn interfaces(&self) -> Vec<(windows_core::GUID, WLAN_INTERFACE_STATE)> {
        let mut result = Vec::new();
        unsafe {
            let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
            if WlanEnumInterfaces(self.0, None, &mut list) != 0 || list.is_null() {
                return result;
            }
            let count = (*list).dwNumberOfItems as usize;
            if count <= 64 {
                let items = std::slice::from_raw_parts((*list).InterfaceInfo.as_ptr(), count);
                for item in items {
                    result.push((item.InterfaceGuid, item.isState));
                }
            }
            WlanFreeMemory(list as *const _);
        }
        result
    }

    /// SSID of the network the interface is associated with.
    /// `Err(true)` means Windows denied access (location privacy).
    fn current_ssid(&self, guid: &windows_core::GUID) -> Result<String, bool> {
        unsafe {
            let mut size = 0u32;
            let mut data: *mut std::ffi::c_void = std::ptr::null_mut();
            let ret = WlanQueryInterface(
                self.0,
                guid,
                wlan_intf_opcode_current_connection,
                None,
                &mut size,
                &mut data,
                None,
            );
            if ret != 0 || data.is_null() {
                return Err(ret == ERROR_ACCESS_DENIED);
            }
            let attr = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
            let ssid = ssid_to_string(&attr.wlanAssociationAttributes.dot11Ssid);
            WlanFreeMemory(data);
            Ok(ssid)
        }
    }

    /// Visible networks on one interface. `Err(true)` = access denied.
    fn available_networks(&self, guid: &windows_core::GUID) -> Result<Vec<ScannedNetwork>, bool> {
        let mut result = Vec::new();
        unsafe {
            let mut list: *mut WLAN_AVAILABLE_NETWORK_LIST = std::ptr::null_mut();
            let ret = WlanGetAvailableNetworkList(self.0, guid, 0, None, &mut list);
            if ret != 0 || list.is_null() {
                return Err(ret == ERROR_ACCESS_DENIED);
            }
            let count = (*list).dwNumberOfItems as usize;
            if count <= 1024 {
                let items = std::slice::from_raw_parts((*list).Network.as_ptr(), count);
                for net in items {
                    let ssid = ssid_to_string(&net.dot11Ssid);
                    if ssid.is_empty() {
                        continue; // hidden network
                    }
                    result.push(ScannedNetwork {
                        interface: *guid,
                        ssid,
                        profile_name: wide_to_string(&net.strProfileName),
                        signal_percent: net.wlanSignalQuality.min(100) as u8,
                        is_secure: net.bSecurityEnabled.as_bool(),
                        is_connected: net.dwFlags & WLAN_AVAILABLE_NETWORK_CONNECTED != 0,
                        has_profile: net.dwFlags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE != 0,
                        bss_type: net.dot11BssType,
                        auth: net.dot11DefaultAuthAlgorithm,
                        cipher: net.dot11DefaultCipherAlgorithm,
                    });
                }
            }
            WlanFreeMemory(list as *const _);
        }
        Ok(result)
    }
}

impl Drop for WlanClient {
    fn drop(&mut self) {
        unsafe {
            WlanCloseHandle(self.0, None);
        }
    }
}

#[derive(Clone)]
struct ScannedNetwork {
    interface: windows_core::GUID,
    ssid: String,
    profile_name: String,
    signal_percent: u8,
    is_secure: bool,
    is_connected: bool,
    has_profile: bool,
    bss_type: DOT11_BSS_TYPE,
    auth: DOT11_AUTH_ALGORITHM,
    cipher: DOT11_CIPHER_ALGORITHM,
}

fn ssid_to_string(ssid: &DOT11_SSID) -> String {
    let len = (ssid.uSSIDLength as usize).min(32);
    String::from_utf8_lossy(&ssid.ucSSID[..len])
        .trim_end_matches('\0')
        .trim()
        .to_string()
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Current Wi-Fi association, read without needing location permission.
pub struct WlanSnapshot {
    /// At least one adapter is powered and ready.
    pub enabled: bool,
    pub connected: bool,
    /// Empty when not connected or when Windows hides it (location off).
    pub ssid: String,
}

pub fn snapshot() -> WlanSnapshot {
    let mut snap = WlanSnapshot {
        enabled: false,
        connected: false,
        ssid: String::new(),
    };
    let Ok(client) = WlanClient::open() else {
        return snap;
    };
    for (guid, state) in client.interfaces() {
        if state != wlan_interface_state_not_ready {
            snap.enabled = true;
        }
        if state == wlan_interface_state_connected {
            snap.connected = true;
            snap.ssid = client.current_ssid(&guid).unwrap_or_default();
            break;
        }
    }
    snap
}

/// Merges per-interface/per-profile duplicates into one row per SSID.
fn merge_networks(scanned: &[ScannedNetwork]) -> Vec<WifiNetworkEntry> {
    let mut out: Vec<WifiNetworkEntry> = Vec::new();
    for net in scanned {
        if let Some(existing) = out.iter_mut().find(|n| n.ssid == net.ssid) {
            existing.signal_percent = existing.signal_percent.max(net.signal_percent);
            existing.is_connected |= net.is_connected;
            existing.has_profile |= net.has_profile;
            existing.is_secure |= net.is_secure;
        } else {
            out.push(WifiNetworkEntry {
                ssid: net.ssid.clone(),
                signal_percent: net.signal_percent,
                is_secure: net.is_secure,
                is_connected: net.is_connected,
                has_profile: net.has_profile,
            });
        }
    }
    out.sort_by(|a, b| {
        b.is_connected
            .cmp(&a.is_connected)
            .then_with(|| b.signal_percent.cmp(&a.signal_percent))
            .then_with(|| a.ssid.to_lowercase().cmp(&b.ssid.to_lowercase()))
    });
    out
}

fn list_all(client: &WlanClient) -> Result<Vec<ScannedNetwork>, String> {
    let mut all = Vec::new();
    let mut denied = false;
    for (guid, _) in client.interfaces() {
        match client.available_networks(&guid) {
            Ok(mut nets) => all.append(&mut nets),
            Err(access_denied) => denied |= access_denied,
        }
    }
    if all.is_empty() && denied {
        return Err(LOCATION_PERMISSION_ERROR.to_string());
    }
    Ok(all)
}

/// Triggers a fresh scan, waits for the driver to report, and lists networks.
pub fn scan_networks() -> Result<Vec<WifiNetworkEntry>, String> {
    let client = WlanClient::open()?;
    let interfaces = client.interfaces();
    if interfaces.is_empty() {
        return Err("No Wi-Fi adapter found".into());
    }
    for (guid, state) in &interfaces {
        if *state != wlan_interface_state_not_ready {
            unsafe {
                let _ = WlanScan(client.0, guid, None, None, None);
            }
        }
    }
    // Scans complete within ~4 s by contract; most drivers report in 1–2 s.
    std::thread::sleep(std::time::Duration::from_millis(SCAN_SETTLE_MS));
    Ok(merge_networks(&list_all(&client)?))
}

/// Security settings for a new profile, derived from what the AP advertises.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ProfileSecurity {
    Open,
    WpaPsk { aes: bool },
    Wpa2Psk { aes: bool },
    Wpa3Sae,
}

fn profile_security(
    is_secure: bool,
    auth: DOT11_AUTH_ALGORITHM,
    cipher: DOT11_CIPHER_ALGORITHM,
) -> Result<ProfileSecurity, String> {
    let aes = cipher != DOT11_CIPHER_ALGO_TKIP;
    if !is_secure || (auth == DOT11_AUTH_ALGO_80211_OPEN && cipher == DOT11_CIPHER_ALGO_NONE) {
        return Ok(ProfileSecurity::Open);
    }
    match auth {
        a if a == DOT11_AUTH_ALGO_RSNA_PSK => Ok(ProfileSecurity::Wpa2Psk { aes }),
        a if a == DOT11_AUTH_ALGO_WPA_PSK => Ok(ProfileSecurity::WpaPsk { aes }),
        a if a == DOT11_AUTH_ALGO_WPA3_SAE => Ok(ProfileSecurity::Wpa3Sae),
        _ => Err(
            "This network needs enterprise sign-in. Open Windows Wi-Fi settings to join it.".into(),
        ),
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn build_profile_xml(
    ssid: &str,
    security: ProfileSecurity,
    password: Option<&str>,
) -> Result<String, String> {
    let name = xml_escape(ssid);
    let hex: String = ssid.bytes().map(|b| format!("{b:02X}")).collect();
    let (auth, enc) = match security {
        ProfileSecurity::Open => ("open", "none"),
        ProfileSecurity::WpaPsk { aes } => ("WPAPSK", if aes { "AES" } else { "TKIP" }),
        ProfileSecurity::Wpa2Psk { aes } => ("WPA2PSK", if aes { "AES" } else { "TKIP" }),
        ProfileSecurity::Wpa3Sae => ("WPA3SAE", "AES"),
    };
    let shared_key = if security == ProfileSecurity::Open {
        String::new()
    } else {
        let key = password.unwrap_or_default();
        let len = key.chars().count();
        if !(8..=63).contains(&len) {
            return Err("The password must be 8 to 63 characters".into());
        }
        format!(
            "<sharedKey><keyType>passPhrase</keyType><protected>false</protected><keyMaterial>{}</keyMaterial></sharedKey>",
            xml_escape(key)
        )
    };
    Ok(format!(
        r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
<name>{name}</name>
<SSIDConfig><SSID><hex>{hex}</hex><name>{name}</name></SSID></SSIDConfig>
<connectionType>ESS</connectionType>
<connectionMode>auto</connectionMode>
<MSM><security><authEncryption><authentication>{auth}</authentication><encryption>{enc}</encryption><useOneX>false</useOneX></authEncryption>{shared_key}</security></MSM>
</WLANProfile>"#
    ))
}

/// Joins `ssid`, creating a Windows profile when a password is supplied or the
/// network was never joined before. Returns `Ok(false)` if association did not
/// complete in time (usually a wrong password).
pub fn connect(ssid: &str, password: Option<&str>) -> Result<bool, String> {
    let client = WlanClient::open()?;
    let networks = list_all(&client)?;
    let target = networks
        .iter()
        .filter(|n| n.ssid == ssid)
        .max_by_key(|n| (n.has_profile, n.signal_percent))
        .cloned()
        .ok_or_else(|| "Network not found. Refresh the list and try again.".to_string())?;

    let password = password.filter(|p| !p.is_empty());
    let must_create_profile = password.is_some() || !target.has_profile;
    let profile_name = if must_create_profile {
        let security = profile_security(target.is_secure, target.auth, target.cipher)?;
        if security != ProfileSecurity::Open && password.is_none() {
            return Err("This network needs a password".into());
        }
        let xml = to_wide(&build_profile_xml(ssid, security, password)?);
        let mut reason = 0u32;
        let ret = unsafe {
            WlanSetProfile(
                client.0,
                &target.interface,
                0,
                PCWSTR(xml.as_ptr()),
                PCWSTR::null(),
                true,
                None,
                &mut reason,
            )
        };
        if ret != 0 {
            return Err(format!(
                "Windows rejected the network profile (error {ret}, reason {reason})"
            ));
        }
        ssid.to_string()
    } else if target.profile_name.is_empty() {
        ssid.to_string()
    } else {
        target.profile_name.clone()
    };

    let profile_wide = to_wide(&profile_name);
    let params = WLAN_CONNECTION_PARAMETERS {
        wlanConnectionMode: wlan_connection_mode_profile,
        strProfile: PCWSTR(profile_wide.as_ptr()),
        pDot11Ssid: std::ptr::null_mut(),
        pDesiredBssidList: std::ptr::null_mut(),
        dot11BssType: target.bss_type,
        dwFlags: 0,
    };
    let ret = unsafe { WlanConnect(client.0, &target.interface, &params, None) };
    if ret != 0 {
        return Err(format!("Could not start the connection (error {ret})"));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(CONNECT_TIMEOUT_MS);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let state = client
            .interfaces()
            .into_iter()
            .find(|(guid, _)| *guid == target.interface)
            .map(|(_, s)| s);
        if state == Some(wlan_interface_state_connected) {
            // An unreadable SSID (location off) still counts: the interface
            // only reports connected after WlanConnect succeeded.
            match client.current_ssid(&target.interface) {
                Ok(current) if current != ssid => continue,
                _ => return Ok(true),
            }
        }
    }

    // A profile created with a wrong password would make later attempts skip
    // the password prompt, so remove it again.
    if must_create_profile {
        unsafe {
            let _ = WlanDeleteProfile(
                client.0,
                &target.interface,
                PCWSTR(profile_wide.as_ptr()),
                None,
            );
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escapes_special_characters() {
        assert_eq!(xml_escape(r#"a&b<c>"d'"#), "a&amp;b&lt;c&gt;&quot;d&apos;");
    }

    #[test]
    fn profile_xml_contains_hex_ssid_and_key() {
        let xml = build_profile_xml(
            "Café & Co",
            ProfileSecurity::Wpa2Psk { aes: true },
            Some("secret123"),
        )
        .unwrap();
        assert!(xml.contains("<name>Café &amp; Co</name>"));
        assert!(xml.contains("<hex>436166C3A9202620436F</hex>"));
        assert!(
            xml.contains("<authentication>WPA2PSK</authentication><encryption>AES</encryption>")
        );
        assert!(xml.contains("<keyMaterial>secret123</keyMaterial>"));
    }

    #[test]
    fn open_profile_has_no_key() {
        let xml = build_profile_xml("Guest", ProfileSecurity::Open, None).unwrap();
        assert!(xml.contains("<authentication>open</authentication><encryption>none</encryption>"));
        assert!(!xml.contains("sharedKey"));
    }

    #[test]
    fn rejects_bad_password_length() {
        assert!(build_profile_xml("x", ProfileSecurity::Wpa3Sae, Some("short")).is_err());
        assert!(build_profile_xml("x", ProfileSecurity::Wpa3Sae, None).is_err());
    }

    #[test]
    fn maps_advertised_security() {
        assert_eq!(
            profile_security(true, DOT11_AUTH_ALGO_RSNA_PSK, DOT11_CIPHER_ALGO_CCMP),
            Ok(ProfileSecurity::Wpa2Psk { aes: true })
        );
        assert_eq!(
            profile_security(true, DOT11_AUTH_ALGO_WPA_PSK, DOT11_CIPHER_ALGO_TKIP),
            Ok(ProfileSecurity::WpaPsk { aes: false })
        );
        assert_eq!(
            profile_security(false, DOT11_AUTH_ALGO_80211_OPEN, DOT11_CIPHER_ALGO_NONE),
            Ok(ProfileSecurity::Open)
        );
        assert!(profile_security(true, DOT11_AUTH_ALGO_RSNA, DOT11_CIPHER_ALGO_CCMP).is_err());
    }
}
