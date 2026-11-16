use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

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
fn connect_wmi_root() -> Option<windows::Win32::System::Wmi::IWbemServices> {
    use windows::core::{IUnknown, Interface, BSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoSetProxyBlanket, CLSCTX_INPROC_SERVER, EOAC_NONE,
        RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
    };
    use windows::Win32::System::Wmi::{IWbemLocator, IWbemServices, WbemLocator};

    const RPC_C_AUTHN_WINNT: u32 = 10;
    const RPC_C_AUTHZ_NONE: u32 = 0;

    unsafe {
        let locator: IWbemLocator =
            CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
        let services: IWbemServices = locator
            .ConnectServer(
                &BSTR::from("ROOT\\WMI"),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                0,
                &BSTR::new(),
                None,
            )
            .ok()?;
        if let Ok(unk) = services.cast::<IUnknown>() {
            let _ = CoSetProxyBlanket(
                &unk,
                RPC_C_AUTHN_WINNT,
                RPC_C_AUTHZ_NONE,
                None,
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            );
        }
        Some(services)
    }
}

#[cfg(target_os = "windows")]
fn set_wmi_brightness(clamped: u32) -> bool {
    use windows::core::{w, BSTR, VARIANT};
    use windows::Win32::System::Wmi::{WBEM_FLAG_FORWARD_ONLY, WBEM_INFINITE};

    let _com = ComGuard::init();
    let Some(services) = connect_wmi_root() else {
        return false;
    };

    unsafe {
        let Ok(enumerator) = services.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from("SELECT __PATH FROM WmiMonitorBrightnessMethods"),
            WBEM_FLAG_FORWARD_ONLY,
            None,
        ) else {
            return false;
        };

        let mut objects = [None];
        let mut returned = 0u32;
        if enumerator
            .Next(WBEM_INFINITE, &mut objects, &mut returned)
            .is_err()
            || returned == 0
        {
            return false;
        }
        let Some(instance) = objects[0].take() else {
            return false;
        };

        let mut path_var = VARIANT::new();
        if instance
            .Get(w!("__PATH"), 0, &mut path_var, None, None)
            .is_err()
        {
            return false;
        }
        let Ok(obj_path) = BSTR::try_from(&path_var) else {
            return false;
        };

        let mut class_obj = None;
        if services
            .GetObject(
                &BSTR::from("WmiMonitorBrightnessMethods"),
                Default::default(),
                None,
                Some(&mut class_obj),
                None,
            )
            .is_err()
        {
            return false;
        }
        let Some(class_obj) = class_obj else {
            return false;
        };

        let mut in_sig = None;
        if class_obj
            .GetMethod(w!("WmiSetBrightness"), 0, &mut in_sig, std::ptr::null_mut())
            .is_err()
        {
            return false;
        }
        let Some(in_sig) = in_sig else {
            return false;
        };

        let Ok(in_params) = in_sig.SpawnInstance(0) else {
            return false;
        };
        let timeout_var = VARIANT::from(1u32);
        let brightness_var = VARIANT::from((clamped.min(100)) as u8);
        if in_params.Put(w!("Timeout"), 0, &timeout_var, 0).is_err()
            || in_params
                .Put(w!("Brightness"), 0, &brightness_var, 0)
                .is_err()
        {
            return false;
        }

        services
            .ExecMethod(
                &obj_path,
                &BSTR::from("WmiSetBrightness"),
                Default::default(),
                None,
                &in_params,
                None,
                None,
            )
            .is_ok()
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
    let skip_wmi = SLOW_SYS_CACHE
        .lock()
        .ok()
        .is_some_and(|c| c.wmi_retry_after.is_some_and(|t| Instant::now() < t));

    // Try native COM WMI internal panel brightness first on laptops unless known unsupported
    if !skip_wmi && set_wmi_brightness(clamped) {
        cache_brightness_value(Some(clamped));
        return Ok(());
    }

    // Fallback to native DDC/CI physical monitor brightness
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
                    cache_brightness_value(Some(clamped));
                    return Ok(());
                }
            }
        }
    }

    Err("Display brightness control is unavailable on this monitor (enable DDC/CI or use an internal display)".into())
}

#[derive(Default)]
struct SlowSysCache {
    brightness: Option<u32>,
    brightness_at: Option<Instant>,
    wmi_retry_after: Option<Instant>,
    wifi: (bool, Option<String>),
    wifi_at: Option<Instant>,
}

static SLOW_SYS_CACHE: Mutex<SlowSysCache> = Mutex::new(SlowSysCache {
    brightness: None,
    brightness_at: None,
    wmi_retry_after: None,
    wifi: (false, None),
    wifi_at: None,
});

#[cfg(target_os = "windows")]
fn cache_brightness_value(val: Option<u32>) {
    if let Ok(mut cache) = SLOW_SYS_CACHE.lock() {
        cache.brightness = val;
        cache.brightness_at = Some(Instant::now());
    }
}

#[cfg(target_os = "windows")]
fn cached_query_brightness(force_refresh: bool) -> Option<u32> {
    let now = Instant::now();
    if let Ok(cache) = SLOW_SYS_CACHE.lock() {
        if let Some(at) = cache.brightness_at {
            let ttl = if force_refresh {
                Duration::from_secs(12)
            } else {
                Duration::from_secs(20)
            };
            if now.duration_since(at) < ttl {
                return cache.brightness;
            }
        }
    }

    // Native DDC/CI has zero process-spawn overhead, check it first if WMI previously returned None.
    let wmi_blocked = SLOW_SYS_CACHE
        .lock()
        .ok()
        .is_some_and(|c| c.wmi_retry_after.is_some_and(|t| now < t));

    let val = if wmi_blocked {
        query_ddc_brightness()
    } else if let Some(wmi) = query_wmi_brightness() {
        Some(wmi)
    } else {
        if let Ok(mut cache) = SLOW_SYS_CACHE.lock() {
            cache.wmi_retry_after = Some(Instant::now() + Duration::from_secs(120));
        }
        query_ddc_brightness()
    };

    if let Ok(mut cache) = SLOW_SYS_CACHE.lock() {
        cache.brightness = val;
        cache.brightness_at = Some(Instant::now());
    }
    val
}

#[cfg(target_os = "windows")]
fn cached_query_wifi(force_refresh: bool) -> (bool, Option<String>) {
    let now = Instant::now();
    if let Ok(cache) = SLOW_SYS_CACHE.lock() {
        if let Some(at) = cache.wifi_at {
            let ttl = if force_refresh {
                Duration::from_secs(15)
            } else {
                Duration::from_secs(30)
            };
            if now.duration_since(at) < ttl {
                return cache.wifi.clone();
            }
        }
    }
    if !force_refresh
        && SLOW_SYS_CACHE
            .lock()
            .ok()
            .is_some_and(|c| c.wifi_at.is_some())
    {
        if let Ok(cache) = SLOW_SYS_CACHE.lock() {
            return cache.wifi.clone();
        }
    }
    let res = query_wifi_status();
    if let Ok(mut cache) = SLOW_SYS_CACHE.lock() {
        cache.wifi = res.clone();
        cache.wifi_at = Some(Instant::now());
    }
    res
}

#[cfg(target_os = "windows")]
fn query_wmi_brightness() -> Option<u32> {
    use windows::core::{w, BSTR, VARIANT};
    use windows::Win32::System::Wmi::{WBEM_FLAG_FORWARD_ONLY, WBEM_INFINITE};

    let _com = ComGuard::init();
    let services = connect_wmi_root()?;
    unsafe {
        let enumerator = services
            .ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from("SELECT CurrentBrightness FROM WmiMonitorBrightness"),
                WBEM_FLAG_FORWARD_ONLY,
                None,
            )
            .ok()?;

        let mut objects = [None];
        let mut returned = 0u32;
        if enumerator
            .Next(WBEM_INFINITE, &mut objects, &mut returned)
            .is_err()
            || returned == 0
        {
            return None;
        }
        let instance = objects[0].take()?;
        let mut val = VARIANT::new();
        instance
            .Get(w!("CurrentBrightness"), 0, &mut val, None, None)
            .ok()?;
        u32::try_from(&val).ok().map(|v| v.min(100))
    }
}

/// Checks native Wi-Fi link state via Windows IP Helper (`GetIfTable2`) for
/// `IF_TYPE_IEEE80211` (71) without spawning `netsh.exe` or querying Location-gated WLAN SSIDs.
#[cfg(target_os = "windows")]
fn query_wifi_status() -> (bool, Option<String>) {
    use windows::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2,
    };
    use windows::Win32::NetworkManagement::Ndis::{IfOperStatusUp, MediaConnectStateConnected};

    const IF_TYPE_IEEE80211: u32 = 71;
    let mut connected = false;
    let mut adapter_label = None;

    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return (false, None);
        }

        let count = (*table).NumEntries as usize;
        let rows: &[MIB_IF_ROW2] = std::slice::from_raw_parts((*table).Table.as_ptr(), count);
        for row in rows {
            if row.Type != IF_TYPE_IEEE80211 {
                continue;
            }
            if (row.InterfaceAndOperStatusFlags._bitfield & 0b10) != 0 {
                continue;
            }
            if row.OperStatus == IfOperStatusUp
                && row.MediaConnectState == MediaConnectStateConnected
            {
                connected = true;
                let len = row
                    .Alias
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(row.Alias.len());
                let alias = String::from_utf16_lossy(&row.Alias[..len])
                    .trim()
                    .to_string();
                if !alias.is_empty() {
                    adapter_label = Some(alias);
                }
                break;
            }
        }

        FreeMibTable(table as *const _);
    }

    (connected, adapter_label)
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
        let val = cached_query_brightness(include_slow_queries);
        *cached_brightness = Some(val);
        val
    } else if let Ok(c) = SLOW_SYS_CACHE.lock() {
        c.brightness.or_else(|| cached_brightness.unwrap_or(None))
    } else {
        cached_brightness.unwrap_or(None)
    };

    let output_devices = list_audio_devices(eRender, "output");
    let input_devices = list_audio_devices(eCapture, "input");
    let battery = query_battery();
    let (wifi_connected, wifi_ssid) = cached_query_wifi(include_slow_queries);

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

/// Spawns an event-driven WASAPI listener thread using `IAudioEndpointVolumeCallback` and
/// `IMMNotificationClient` so volume changes, mute toggles, and default device switches
/// wake `notify` immediately with zero polling.
pub fn spawn_audio_event_listener(notify: Arc<Notify>) {
    #[cfg(target_os = "windows")]
    {
        std::thread::Builder::new()
            .name("pillvyn-wasapi-events".into())
            .spawn(move || unsafe {
                use std::sync::mpsc;
                use windows::core::{implement, PCWSTR};
                use windows::Win32::Media::Audio::Endpoints::{
                    IAudioEndpointVolume, IAudioEndpointVolumeCallback,
                    IAudioEndpointVolumeCallback_Impl,
                };
                use windows::Win32::Media::Audio::{
                    eCapture, eConsole, eMultimedia, eRender, EDataFlow, ERole,
                    IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
                    MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE,
                };
                use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
                use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

                #[implement(IAudioEndpointVolumeCallback)]
                struct VolumeCb {
                    notify: Arc<Notify>,
                }

                impl IAudioEndpointVolumeCallback_Impl for VolumeCb_Impl {
                    fn OnNotify(
                        &self,
                        _pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA,
                    ) -> windows::core::Result<()> {
                        self.notify.notify_one();
                        Ok(())
                    }
                }

                #[implement(IMMNotificationClient)]
                struct DeviceClient {
                    notify: Arc<Notify>,
                    rebind_tx: mpsc::Sender<()>,
                }

                impl IMMNotificationClient_Impl for DeviceClient_Impl {
                    fn OnDeviceStateChanged(
                        &self,
                        _pwstrdeviceid: &PCWSTR,
                        _dwnewstate: DEVICE_STATE,
                    ) -> windows::core::Result<()> {
                        self.notify.notify_one();
                        let _ = self.rebind_tx.send(());
                        Ok(())
                    }

                    fn OnDeviceAdded(&self, _pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
                        self.notify.notify_one();
                        let _ = self.rebind_tx.send(());
                        Ok(())
                    }

                    fn OnDeviceRemoved(
                        &self,
                        _pwstrdeviceid: &PCWSTR,
                    ) -> windows::core::Result<()> {
                        self.notify.notify_one();
                        let _ = self.rebind_tx.send(());
                        Ok(())
                    }

                    fn OnDefaultDeviceChanged(
                        &self,
                        _flow: EDataFlow,
                        role: ERole,
                        _pwstrdefaultdeviceid: &PCWSTR,
                    ) -> windows::core::Result<()> {
                        if role == eMultimedia || role == eConsole {
                            self.notify.notify_one();
                            let _ = self.rebind_tx.send(());
                        }
                        Ok(())
                    }

                    fn OnPropertyValueChanged(
                        &self,
                        _pwstrdeviceid: &PCWSTR,
                        _key: &PROPERTYKEY,
                    ) -> windows::core::Result<()> {
                        Ok(())
                    }
                }

                let _com = ComGuard::init();
                let Ok(enumerator): windows::core::Result<IMMDeviceEnumerator> =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                else {
                    return;
                };

                let (rebind_tx, rebind_rx) = mpsc::channel::<()>();
                let client: IMMNotificationClient = DeviceClient {
                    notify: Arc::clone(&notify),
                    rebind_tx: rebind_tx.clone(),
                }
                .into();
                let _ = enumerator.RegisterEndpointNotificationCallback(&client);

                let bind_flow = |flow: EDataFlow| -> Option<(
                    IAudioEndpointVolume,
                    IAudioEndpointVolumeCallback,
                )> {
                    let dev = enumerator.GetDefaultAudioEndpoint(flow, eMultimedia).ok()?;
                    let vol: IAudioEndpointVolume = dev.Activate(CLSCTX_ALL, None).ok()?;
                    let cb: IAudioEndpointVolumeCallback = VolumeCb {
                        notify: Arc::clone(&notify),
                    }
                    .into();
                    vol.RegisterControlChangeNotify(&cb).ok()?;
                    Some((vol, cb))
                };

                let mut bound_out = bind_flow(eRender);
                let mut bound_in = bind_flow(eCapture);

                while rebind_rx.recv().is_ok() {
                    while rebind_rx.try_recv().is_ok() {}
                    if let Some((vol, cb)) = bound_out.take() {
                        let _ = vol.UnregisterControlChangeNotify(&cb);
                    }
                    if let Some((vol, cb)) = bound_in.take() {
                        let _ = vol.UnregisterControlChangeNotify(&cb);
                    }
                    bound_out = bind_flow(eRender);
                    bound_in = bind_flow(eCapture);
                    notify.notify_one();
                }

                let _ = enumerator.UnregisterEndpointNotificationCallback(&client);
            })
            .ok();
    }

    #[cfg(not(target_os = "windows"))]
    let _ = notify;
}
