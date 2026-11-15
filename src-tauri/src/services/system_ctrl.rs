use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub kind: String, // "output" | "input"
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatteryStatus {
    pub present: bool,
    pub ac_online: bool,
    pub charging: bool,
    pub percent: Option<u8>,
    pub battery_saver: bool,
    pub remaining_seconds: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemControlsSnapshot {
    pub output_volume: f32,
    pub output_muted: bool,
    pub mic_volume: f32,
    pub mic_muted: bool,
    pub mic_available: bool,
    pub brightness: Option<u32>,
    pub brightness_supported: bool,
    pub battery: BatteryStatus,
    pub output_devices: Vec<AudioDeviceInfo>,
    pub input_devices: Vec<AudioDeviceInfo>,
    pub wifi_connected: bool,
    pub wifi_ssid: Option<String>,
    pub bluetooth_available: bool,
}

#[cfg(target_os = "windows")]
struct ComGuard {
    initialized: bool,
}

#[cfg(target_os = "windows")]
impl ComGuard {
    fn init() -> Self {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self {
            initialized: hr.is_ok(),
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialized {
            use windows::Win32::System::Com::CoUninitialize;
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn read_endpoint_volume(flow: windows::Win32::Media::Audio::EDataFlow) -> Option<(f32, bool)> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{eMultimedia, IMMDeviceEnumerator, MMDeviceEnumerator};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    let _com = ComGuard::init();
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(flow, eMultimedia).ok()?;
        let vol: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).ok()?;
        let level = vol.GetMasterVolumeLevelScalar().ok()?;
        let muted = vol.GetMute().ok()?.as_bool();
        Some((level.clamp(0.0, 1.0), muted))
    }
}

#[cfg(target_os = "windows")]
pub fn set_endpoint_volume(is_input: bool, level: f32) -> Result<(), String> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eCapture, eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    let _com = ComGuard::init();
    let flow = if is_input { eCapture } else { eRender };
    let clamped = level.clamp(0.0, 1.0);
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("Audio enumerator unavailable: {e}"))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(flow, eMultimedia)
            .map_err(|e| format!("Default audio endpoint unavailable: {e}"))?;
        let vol: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Endpoint volume control unavailable: {e}"))?;
        vol.SetMasterVolumeLevelScalar(clamped, std::ptr::null())
            .map_err(|e| format!("Failed to set volume: {e}"))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn set_endpoint_mute(is_input: bool, muted: bool) -> Result<(), String> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eCapture, eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    let _com = ComGuard::init();
    let flow = if is_input { eCapture } else { eRender };
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("Audio enumerator unavailable: {e}"))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(flow, eMultimedia)
            .map_err(|e| format!("Default audio endpoint unavailable: {e}"))?;
        let vol: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Endpoint volume control unavailable: {e}"))?;
        vol.SetMute(muted, std::ptr::null())
            .map_err(|e| format!("Failed to toggle mute: {e}"))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn list_audio_devices(
    flow: windows::Win32::Media::Audio::EDataFlow,
    kind: &str,
) -> Vec<AudioDeviceInfo> {
    use windows::core::GUID;
    use windows::Win32::Media::Audio::{
        eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};
    use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

    const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };

    let _com = ComGuard::init();
    let mut list = Vec::new();
    unsafe {
        let Ok(enumerator): Result<IMMDeviceEnumerator, _> =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
        else {
            return list;
        };

        let default_id = enumerator
            .GetDefaultAudioEndpoint(flow, eMultimedia)
            .ok()
            .and_then(|d| d.GetId().ok())
            .and_then(|pw| {
                let s = pw.to_string().ok();
                CoTaskMemFree(Some(pw.0 as *const _));
                s
            })
            .unwrap_or_default();

        let Ok(collection) = enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE) else {
            return list;
        };
        let count = collection.GetCount().unwrap_or(0);
        for i in 0..count {
            let Ok(dev) = collection.Item(i) else {
                continue;
            };
            let id = dev
                .GetId()
                .ok()
                .and_then(|pw| {
                    let s = pw.to_string().ok();
                    CoTaskMemFree(Some(pw.0 as *const _));
                    s
                })
                .unwrap_or_default();
            let name = dev
                .OpenPropertyStore(STGM_READ)
                .ok()
                .and_then(|store| store.GetValue(&PKEY_DEVICE_FRIENDLY_NAME).ok())
                .and_then(|pv| {
                    PropVariantToStringAlloc(&pv).ok().and_then(|pw| {
                        let s = pw.to_string().ok();
                        CoTaskMemFree(Some(pw.0 as *const _));
                        s
                    })
                })
                .unwrap_or_else(|| {
                    if flow == eRender {
                        format!("Output Device {}", i + 1)
                    } else {
                        format!("Input Device {}", i + 1)
                    }
                });
            if !id.is_empty() {
                list.push(AudioDeviceInfo {
                    is_default: id == default_id,
                    id,
                    name,
                    kind: kind.to_string(),
                });
            }
        }
    }
    list
}

/// Switches the Windows default audio endpoint using COM `IPolicyConfig`.
#[cfg(target_os = "windows")]
pub fn set_default_audio_device(device_id: &str) -> Result<(), String> {
    use std::ffi::c_void;
    use windows::core::{GUID, HRESULT, HSTRING, PCWSTR};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
    const IID_IPOLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);

    #[repr(C)]
    struct PolicyConfigVtbl {
        // IUnknown
        query_interface:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        // IPolicyConfig slots 3..12 before SetDefaultEndpoint (slot 13)
        _slots: [usize; 10],
        set_default_endpoint: unsafe extern "system" fn(*mut c_void, PCWSTR, u32) -> HRESULT,
    }

    #[repr(C)]
    struct PolicyConfigRaw {
        vtbl: *const PolicyConfigVtbl,
    }

    let _com = ComGuard::init();
    let wide = HSTRING::from(device_id);
    unsafe {
        let unknown: windows::core::IUnknown =
            CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL)
                .map_err(|e| format!("Audio routing service unavailable: {e}"))?;
        let mut raw_ptr: *mut c_void = std::ptr::null_mut();
        use windows::core::Interface;
        let hr = unknown.query(&IID_IPOLICY_CONFIG, &mut raw_ptr);
        if hr.is_err() || raw_ptr.is_null() {
            return Err("IPolicyConfig interface unsupported on this Windows build".into());
        }
        let iface = raw_ptr as *mut PolicyConfigRaw;
        // Set for Console (0), Multimedia (1), and Communications (2)
        for role in 0u32..=2u32 {
            let res = ((*(*iface).vtbl).set_default_endpoint)(raw_ptr, PCWSTR(wide.as_ptr()), role);
            if res.is_err() {
                ((*(*iface).vtbl).release)(raw_ptr);
                return Err(format!(
                    "Failed to set default audio endpoint (hr={:?})",
                    res
                ));
            }
        }
        ((*(*iface).vtbl).release)(raw_ptr);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn query_battery() -> BatteryStatus {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut sps = SYSTEM_POWER_STATUS::default();
    let ok = unsafe { GetSystemPowerStatus(&mut sps).is_ok() };
    if !ok || (sps.BatteryFlag & 128) != 0 {
        return BatteryStatus {
            present: false,
            ac_online: sps.ACLineStatus == 1,
            charging: false,
            percent: None,
            battery_saver: sps.SystemStatusFlag == 1,
            remaining_seconds: None,
        };
    }

    let percent = if sps.BatteryLifePercent <= 100 {
        Some(sps.BatteryLifePercent)
    } else {
        None
    };
    let remaining_seconds = if sps.BatteryLifeTime != u32::MAX {
        Some(sps.BatteryLifeTime)
    } else {
        None
    };

    BatteryStatus {
        present: true,
        ac_online: sps.ACLineStatus == 1,
        charging: (sps.BatteryFlag & 8) != 0,
        percent,
        battery_saver: sps.SystemStatusFlag == 1,
        remaining_seconds,
    }
}

#[cfg(target_os = "windows")]
fn query_ddc_brightness() -> Option<u32> {
    use windows::Win32::Devices::Display::{
        DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
        GetPhysicalMonitorsFromHMONITOR, PHYSICAL_MONITOR,
    };
    use windows::Win32::Foundation::{BOOL, LPARAM, POINT, RECT};
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, HMONITOR, MONITOR_DEFAULTTOPRIMARY};

    unsafe {
        let hmon: HMONITOR = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut count = 0u32;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count).is_err() || count == 0 {
            return None;
        }
        let mut monitors = vec![PHYSICAL_MONITOR::default(); count as usize];
        if GetPhysicalMonitorsFromHMONITOR(hmon, &mut monitors).is_err() {
            return None;
        }
        let mut min_b = 0u32;
        let mut cur_b = 0u32;
        let mut max_b = 0u32;
        let mut out = None;
        if GetMonitorBrightness(
            monitors[0].hPhysicalMonitor,
            &mut min_b,
            &mut cur_b,
            &mut max_b,
        ) != 0
            && max_b > min_b
        {
            let pct = (((cur_b - min_b) as f64 / (max_b - min_b) as f64) * 100.0).round() as u32;
            out = Some(pct.min(100));
        }
        let _ = DestroyPhysicalMonitors(&monitors);
        let _ = (BOOL(1), LPARAM(0), RECT::default());
        out
    }
}

#[cfg(target_os = "windows")]
pub fn set_brightness_percent(percent: u32) -> Result<(), String> {
    use windows::Win32::Devices::Display::{
        DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
        GetPhysicalMonitorsFromHMONITOR, SetMonitorBrightness, PHYSICAL_MONITOR,
    };
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};

    let clamped = percent.min(100);
    // Try WMI internal panel brightness first (laptops)
    let wmi_script = format!(
        "(Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightnessMethods -ErrorAction SilentlyContinue | Select-Object -First 1) | Invoke-CimMethod -MethodName WmiSetBrightness -Arguments @{{Timeout=1; Brightness={clamped}}}"
    );
    if let Ok(out) = hidden_powershell(&wmi_script) {
        if out.status.success() && query_wmi_brightness().is_some() {
            return Ok(());
        }
    }

    // Fallback to DDC/CI physical monitor brightness
    unsafe {
        let hmon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut count = 0u32;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count).is_ok() && count > 0 {
            let mut monitors = vec![PHYSICAL_MONITOR::default(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(hmon, &mut monitors).is_ok() {
                let mut ok = false;
                for m in &monitors {
                    let mut min_b = 0u32;
                    let mut cur_b = 0u32;
                    let mut max_b = 0u32;
                    if GetMonitorBrightness(m.hPhysicalMonitor, &mut min_b, &mut cur_b, &mut max_b)
                        != 0
                        && max_b > min_b
                    {
                        let raw = min_b + (((max_b - min_b) * clamped) / 100);
                        if SetMonitorBrightness(m.hPhysicalMonitor, raw) != 0 {
                            ok = true;
                        }
                    }
                }
                let _ = DestroyPhysicalMonitors(&monitors);
                if ok {
                    return Ok(());
                }
            }
        }
    }

    Err("Display brightness control is unavailable on this monitor (enable DDC/CI or use an internal display)".into())
}

#[cfg(target_os = "windows")]
fn query_wmi_brightness() -> Option<u32> {
    let out = hidden_powershell(
        "(Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightness -ErrorAction SilentlyContinue | Select-Object -First 1).CurrentBrightness",
    )
    .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    text.parse::<u32>().ok().map(|v| v.min(100))
}

#[cfg(target_os = "windows")]
fn hidden_powershell(script: &str) -> std::io::Result<std::process::Output> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

#[cfg(target_os = "windows")]
fn query_wifi_status() -> (bool, Option<String>) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let Ok(out) = Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return (false, None);
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut connected = false;
    let mut ssid = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("State") && trimmed.to_ascii_lowercase().contains("connected") {
            if !trimmed.to_ascii_lowercase().contains("disconnected") {
                connected = true;
            }
        } else if trimmed.starts_with("SSID") && !trimmed.starts_with("BSSID") {
            if let Some((_, val)) = trimmed.split_once(':') {
                let s = val.trim().to_string();
                if !s.is_empty() {
                    ssid = Some(s);
                }
            }
        }
    }
    (connected, ssid)
}

#[cfg(target_os = "windows")]
pub fn query_system_snapshot(
    include_slow_queries: bool,
    cached_brightness: &mut Option<Option<u32>>,
) -> SystemControlsSnapshot {
    use windows::Win32::Media::Audio::{eCapture, eRender};

    let (output_volume, output_muted) = read_endpoint_volume(eRender).unwrap_or((0.0, false));
    let mic = read_endpoint_volume(eCapture);
    let (mic_volume, mic_muted, mic_available) = match mic {
        Some((v, m)) => (v, m, true),
        None => (0.0, true, false),
    };

    let brightness = if include_slow_queries || cached_brightness.is_none() {
        let val = query_wmi_brightness().or_else(query_ddc_brightness);
        *cached_brightness = Some(val);
        val
    } else {
        cached_brightness.unwrap_or(None)
    };

    let output_devices = list_audio_devices(eRender, "output");
    let input_devices = list_audio_devices(eCapture, "input");
    let battery = query_battery();
    let (wifi_connected, wifi_ssid) = if include_slow_queries {
        query_wifi_status()
    } else {
        (false, None)
    };

    SystemControlsSnapshot {
        output_volume,
        output_muted,
        mic_volume,
        mic_muted,
        mic_available,
        brightness,
        brightness_supported: brightness.is_some(),
        battery,
        output_devices,
        input_devices,
        wifi_connected,
        wifi_ssid,
        bluetooth_available: true,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn query_system_snapshot(
    _include_slow: bool,
    _cached: &mut Option<Option<u32>>,
) -> SystemControlsSnapshot {
    SystemControlsSnapshot {
        output_volume: 0.0,
        output_muted: false,
        mic_volume: 0.0,
        mic_muted: true,
        mic_available: false,
        brightness: None,
        brightness_supported: false,
        battery: BatteryStatus {
            present: false,
            ac_online: true,
            charging: false,
            percent: None,
            battery_saver: false,
            remaining_seconds: None,
        },
        output_devices: Vec::new(),
        input_devices: Vec::new(),
        wifi_connected: false,
        wifi_ssid: None,
        bluetooth_available: false,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_endpoint_volume(_is_input: bool, _level: f32) -> Result<(), String> {
    Err("Audio controls require Windows".into())
}

#[cfg(not(target_os = "windows"))]
pub fn set_endpoint_mute(_is_input: bool, _muted: bool) -> Result<(), String> {
    Err("Audio controls require Windows".into())
}

#[cfg(not(target_os = "windows"))]
pub fn set_default_audio_device(_device_id: &str) -> Result<(), String> {
    Err("Audio device switching requires Windows".into())
}

#[cfg(not(target_os = "windows"))]
pub fn set_brightness_percent(_percent: u32) -> Result<(), String> {
    Err("Brightness controls require Windows".into())
}
