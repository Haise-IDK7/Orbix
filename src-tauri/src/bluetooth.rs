//! Bluetooth devices for the notch quick settings: what's paired, what's
//! connected, and connect/disconnect for audio devices.
//!
//! Windows has no public "connect this device" call. For headphones and
//! speakers, the Bluetooth audio driver exposes one-shot reconnect/disconnect
//! properties (`KSPROPSETID_BtAudio`) on the device's kernel-streaming filter;
//! that's what the Sound settings page uses. Other kinds (mice, phones,
//! controllers) are connected by Windows itself, so they're listed with their
//! status only. Pairing new devices goes through Windows' "Add a device".

use serde::Serialize;
use windows::Win32::Devices::Bluetooth::{
    BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindNextDevice,
    BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS,
};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct BtDevice {
    pub id: String,
    pub name: String,
    /// "audio", "input", "phone", "computer" or "other".
    pub kind: String,
    pub connected: bool,
    /// Orbix can connect/disconnect it (Bluetooth audio).
    pub can_toggle: bool,
    /// Battery percentage, when the device reports one (connected only).
    pub battery: Option<u8>,
}

#[derive(Serialize)]
pub struct BtOverview {
    pub enabled: bool,
    pub devices: Vec<BtDevice>,
}

/// Major device class from a Class of Device value (bits 8–12).
fn kind_from_class(cod: u32) -> &'static str {
    match (cod >> 8) & 0x1F {
        1 => "computer",
        2 => "phone",
        4 => "audio",
        5 => "input",
        _ => "other",
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end]).trim().to_string()
}

/// Paired (remembered) classic Bluetooth devices, with live connection state.
fn classic_devices() -> Vec<BtDevice> {
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: true.into(),
        fReturnRemembered: true.into(),
        fReturnUnknown: false.into(),
        fReturnConnected: true.into(),
        fIssueInquiry: false.into(),
        cTimeoutMultiplier: 0,
        hRadio: Default::default(),
    };
    let blank = || BLUETOOTH_DEVICE_INFO {
        dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    let mut out = Vec::new();
    let mut info = blank();
    // Fails with "no more items" when nothing is paired or there's no radio.
    let Ok(find) = (unsafe { BluetoothFindFirstDevice(&params, &mut info) }) else {
        return out;
    };
    loop {
        let name = wide_to_string(&info.szName);
        let kind = kind_from_class(info.ulClassofDevice);
        if !name.is_empty() && (info.fRemembered.as_bool() || info.fAuthenticated.as_bool()) {
            out.push(BtDevice {
                id: format!("bt-{:012x}", unsafe { info.Address.Anonymous.ullLong }),
                name,
                kind: kind.to_string(),
                connected: info.fConnected.as_bool(),
                can_toggle: kind == "audio",
                battery: None,
            });
        }
        info = blank();
        if unsafe { BluetoothFindNextDevice(find, &mut info) }.is_err() {
            break;
        }
    }
    unsafe {
        let _ = BluetoothFindDeviceClose(find);
    }
    out
}

/// Paired Bluetooth LE devices (mice, keyboards, earbuds' LE side).
fn le_devices() -> Vec<BtDevice> {
    use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothLEDevice};
    use windows::Devices::Enumeration::DeviceInformation;
    let Ok(selector) = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true) else {
        return Vec::new();
    };
    let Ok(list) = DeviceInformation::FindAllAsyncAqsFilter(&selector).and_then(|op| op.get())
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for info in list {
        let (Ok(id), Ok(name)) = (info.Id(), info.Name()) else {
            continue;
        };
        let name = name.to_string().trim().to_string();
        if name.is_empty() {
            continue;
        }
        let connected = BluetoothLEDevice::FromIdAsync(&id)
            .and_then(|op| op.get())
            .and_then(|d| d.ConnectionStatus())
            .map(|s| s == BluetoothConnectionStatus::Connected)
            .unwrap_or(false);
        out.push(BtDevice {
            id: id.to_string(),
            name,
            kind: "other".into(),
            connected,
            can_toggle: false,
            battery: None,
        });
    }
    out
}

/// Battery level Windows keeps for a Bluetooth device (the value shown in
/// Settings → Bluetooth & devices). Headsets report it over Hands-Free, LE
/// devices through the Battery Service.
const BATTERY_KEY: windows::Win32::Foundation::DEVPROPKEY =
    windows::Win32::Foundation::DEVPROPKEY {
        fmtid: windows::core::GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5),
        pid: 2,
    };

/// (device-node name, battery %) for every present device that reports one.
fn battery_levels() -> Vec<(String, u8)> {
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
        SetupDiGetDevicePropertyW, DIGCF_ALLCLASSES, DIGCF_PRESENT, SP_DEVINFO_DATA,
    };
    use windows::Win32::Devices::Properties::{
        DEVPKEY_Device_FriendlyName, DEVPROPTYPE, DEVPROP_TYPE_BYTE, DEVPROP_TYPE_STRING,
    };
    let mut out = Vec::new();
    unsafe {
        let Ok(set) = SetupDiGetClassDevsW(
            None,
            windows::core::PCWSTR::null(),
            None,
            DIGCF_ALLCLASSES | DIGCF_PRESENT,
        ) else {
            return out;
        };
        let mut index = 0u32;
        loop {
            let mut data = SP_DEVINFO_DATA {
                cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiEnumDeviceInfo(set, index, &mut data).is_err() {
                break;
            }
            index += 1;
            let mut kind = DEVPROPTYPE(0);
            let mut level = [0u8; 1];
            if SetupDiGetDevicePropertyW(
                set,
                &data,
                &BATTERY_KEY,
                &mut kind,
                Some(&mut level),
                None,
                0,
            )
            .is_err()
                || kind != DEVPROP_TYPE_BYTE
            {
                continue;
            }
            let mut name = [0u16; 256];
            let name_bytes = std::slice::from_raw_parts_mut(
                name.as_mut_ptr() as *mut u8,
                std::mem::size_of_val(&name),
            );
            if SetupDiGetDevicePropertyW(
                set,
                &data,
                &DEVPKEY_Device_FriendlyName,
                &mut kind,
                Some(name_bytes),
                None,
                0,
            )
            .is_ok()
                && kind == DEVPROP_TYPE_STRING
            {
                let name = wide_to_string(&name);
                if !name.is_empty() {
                    out.push((name, level[0].min(100)));
                }
            }
        }
        let _ = SetupDiDestroyDeviceInfoList(set);
    }
    out
}

/// Battery for a device, from the device node named after it ("WH-1000XM4",
/// "WH-1000XM4 Hands-Free AG").
fn battery_for(name: &str, levels: &[(String, u8)]) -> Option<u8> {
    levels
        .iter()
        .find(|(node, _)| names_match(name, std::slice::from_ref(node)))
        .map(|(_, level)| *level)
}

/// Classic first (it knows the device type), then LE devices not already
/// listed under the same name; connected devices on top.
fn merge(classic: Vec<BtDevice>, le: Vec<BtDevice>) -> Vec<BtDevice> {
    let mut out = classic;
    for dev in le {
        if let Some(existing) = out
            .iter_mut()
            .find(|d| d.name.eq_ignore_ascii_case(&dev.name))
        {
            existing.connected |= dev.connected;
        } else {
            out.push(dev);
        }
    }
    out.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

pub fn overview() -> BtOverview {
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }
    let enabled =
        crate::commands::get_radio_state_sync(windows::Devices::Radios::RadioKind::Bluetooth)
            .unwrap_or(false);
    let mut devices = if enabled {
        merge(classic_devices(), le_devices())
    } else {
        Vec::new()
    };
    if devices.iter().any(|d| d.connected) {
        let levels = battery_levels();
        for dev in devices.iter_mut().filter(|d| d.connected) {
            dev.battery = battery_for(&dev.name, &levels);
        }
    }
    BtOverview { enabled, devices }
}

/// The endpoint belongs to this Bluetooth device: Windows names the endpoint's
/// device after it ("Auriculares (WH-1000XM4)" / "WH-1000XM4").
fn names_match(device_name: &str, endpoint_names: &[String]) -> bool {
    let wanted = device_name.to_lowercase();
    endpoint_names.iter().any(|n| {
        let n = n.to_lowercase();
        n == wanted
            || n.contains(&format!("({wanted})"))
            || (wanted.len() >= 4 && n.contains(&wanted))
    })
}

/// Sends the Bluetooth audio driver's one-shot reconnect/disconnect to every
/// audio endpoint (stereo and hands-free) of the named device.
pub fn set_audio_connected(name: &str, connect: bool) -> Result<(), String> {
    use windows::core::Interface;
    use windows::Win32::Devices::FunctionDiscovery::{
        PKEY_DeviceInterface_FriendlyName, PKEY_Device_FriendlyName,
    };
    use windows::Win32::Media::Audio::{
        eCapture, eRender, IConnector, IDeviceTopology, IMMDeviceEnumerator, IPart,
        MMDeviceEnumerator, DEVICE_STATE, DEVICE_STATEMASK_ALL,
    };
    use windows::Win32::Media::KernelStreaming::{
        IKsControl, KSPROPSETID_BtAudio, KSIDENTIFIER, KSIDENTIFIER_0, KSIDENTIFIER_0_0,
        KSPROPERTY_ONESHOT_DISCONNECT, KSPROPERTY_ONESHOT_RECONNECT, KSPROPERTY_TYPE_GET,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
        STGM,
    };

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let property = KSIDENTIFIER {
            Anonymous: KSIDENTIFIER_0 {
                Anonymous: KSIDENTIFIER_0_0 {
                    Set: KSPROPSETID_BtAudio,
                    Id: if connect {
                        KSPROPERTY_ONESHOT_RECONNECT.0 as u32
                    } else {
                        KSPROPERTY_ONESHOT_DISCONNECT.0 as u32
                    },
                    Flags: KSPROPERTY_TYPE_GET,
                },
            },
        };
        let mut done_filters: Vec<String> = Vec::new();
        let mut matched = false;
        for flow in [eRender, eCapture] {
            let Ok(collection) =
                enumerator.EnumAudioEndpoints(flow, DEVICE_STATE(DEVICE_STATEMASK_ALL))
            else {
                continue;
            };
            for i in 0..collection.GetCount().unwrap_or(0) {
                let Ok(endpoint) = collection.Item(i) else {
                    continue;
                };
                let Ok(store) = endpoint.OpenPropertyStore(STGM(0)) else {
                    continue;
                };
                let names: Vec<String> = [
                    &PKEY_DeviceInterface_FriendlyName,
                    &PKEY_Device_FriendlyName,
                ]
                .iter()
                .filter_map(|key| crate::services::endpoint_string(&store, key))
                .collect();
                if !names_match(name, &names) {
                    continue;
                }
                // Endpoint → its connector → the Bluetooth driver's filter.
                let Ok(topology) = endpoint.Activate::<IDeviceTopology>(CLSCTX_ALL, None) else {
                    continue;
                };
                let Ok(connector) = topology.GetConnector(0) else {
                    continue;
                };
                let Ok(other): Result<IConnector, _> = connector.GetConnectedTo() else {
                    continue;
                };
                let Ok(part) = other.cast::<IPart>() else {
                    continue;
                };
                let filter_id = part
                    .GetTopologyObject()
                    .and_then(|t| t.GetDeviceId())
                    .map(|p| {
                        let id = p.to_string().unwrap_or_default();
                        windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
                        id
                    })
                    .unwrap_or_default();
                if !filter_id.is_empty() && done_filters.contains(&filter_id) {
                    continue;
                }
                let mut raw: *mut core::ffi::c_void = std::ptr::null_mut();
                if part
                    .Activate(CLSCTX_INPROC_SERVER.0, &IKsControl::IID, Some(&mut raw))
                    .is_err()
                    || raw.is_null()
                {
                    continue;
                }
                let ks = IKsControl::from_raw(raw);
                let mut returned = 0u32;
                // Fails on non-Bluetooth endpoints; that's how they're skipped.
                if ks
                    .KsProperty(
                        &property,
                        std::mem::size_of::<KSIDENTIFIER>() as u32,
                        std::ptr::null_mut(),
                        0,
                        &mut returned,
                    )
                    .is_ok()
                {
                    matched = true;
                    done_filters.push(filter_id);
                }
            }
        }
        if matched {
            Ok(())
        } else {
            Err(format!(
                "Windows didn't offer a way to {} {name} from here",
                if connect { "connect" } else { "disconnect" }
            ))
        }
    }
}

/// Opens Windows' "Add a device" dialog (Bluetooth pairing).
pub fn open_add_device() {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let uri: Vec<u16> = "ms-settings-connectabledevices:devicediscovery\0"
        .encode_utf16()
        .collect();
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    unsafe {
        let _ = ShellExecuteW(
            None,
            windows::core::PCWSTR(verb.as_ptr()),
            windows::core::PCWSTR(uri.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

#[tauri::command]
pub async fn bluetooth_devices() -> Result<BtOverview, String> {
    tauri::async_runtime::spawn_blocking(overview)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn bluetooth_set_connected(name: String, connected: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || set_audio_connected(&name, connected))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn bluetooth_add_device() {
    open_add_device();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_major_device_class() {
        assert_eq!(kind_from_class(0x240404), "audio"); // headset
        assert_eq!(kind_from_class(0x002540), "input"); // keyboard
        assert_eq!(kind_from_class(0x5a020c), "phone");
        assert_eq!(kind_from_class(0x00010c), "computer");
    }

    #[test]
    fn matches_endpoint_names() {
        let names = vec![
            "Auriculares (WH-1000XM4)".to_string(),
            "WH-1000XM4".to_string(),
        ];
        assert!(names_match("WH-1000XM4", &names));
        assert!(names_match("wh-1000xm4", &names));
        assert!(!names_match("AirPods", &names));
        assert!(!names_match("WH", &["Altavoces (Realtek)".to_string()]));
    }

    #[test]
    fn finds_battery_by_device_node_name() {
        let levels = vec![
            ("Galaxy Buds2 Hands-Free AG".to_string(), 70),
            ("MX Master 3S".to_string(), 95),
        ];
        assert_eq!(battery_for("Galaxy Buds2", &levels), Some(70));
        assert_eq!(battery_for("mx master 3s", &levels), Some(95));
        assert_eq!(battery_for("JBL Flip 5", &levels), None);
    }

    #[test]
    fn merges_and_sorts_connected_first() {
        let dev = |name: &str, connected: bool| BtDevice {
            id: name.into(),
            name: name.into(),
            kind: "other".into(),
            connected,
            can_toggle: false,
            battery: None,
        };
        let merged = merge(
            vec![dev("Buds", false), dev("Alpha", false)],
            vec![dev("buds", true), dev("Mouse", true)],
        );
        let names: Vec<_> = merged
            .iter()
            .map(|d| (d.name.as_str(), d.connected))
            .collect();
        assert_eq!(
            names,
            vec![("Buds", true), ("Mouse", true), ("Alpha", false)]
        );
    }
}
