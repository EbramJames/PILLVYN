use nvml_wrapper::Nvml;
use serde::{Deserialize, Serialize};
use sysinfo::{Components, CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSnapshot {
    pub mount: String,
    pub name: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSnapshot {
    pub name: String,
    pub utilization_percent: Option<u32>,
    pub vram_used_mb: Option<u64>,
    pub vram_total_mb: Option<u64>,
    pub temperature_c: Option<u32>,
    pub power_watts: Option<f32>,
    pub telemetry_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub cpu_brand: String,
    pub cpu_usage_percent: f32,
    pub cpu_cores: usize,
    pub cpu_freq_mhz: u64,
    pub cpu_temp_c: Option<f32>,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub ram_percent: f32,
    pub gpus: Vec<GpuSnapshot>,
    pub disks: Vec<DiskSnapshot>,
}

pub struct HardwareCollector {
    sys: System,
    disks: Disks,
    components: Components,
    nvml: Option<Nvml>,
    #[cfg(target_os = "windows")]
    pdh_gpu: Option<PdhGpuCounter>,
    #[cfg(target_os = "windows")]
    dxgi_adapters: Vec<(String, u64)>,
    sample_count: u64,
}

impl Default for HardwareCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareCollector {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let disks = Disks::new_with_refreshed_list();
        let components = Components::new_with_refreshed_list();
        let nvml = Nvml::init().ok();
        #[cfg(target_os = "windows")]
        let pdh_gpu = PdhGpuCounter::new();
        #[cfg(target_os = "windows")]
        let dxgi_adapters = enumerate_dxgi_adapters();

        Self {
            sys,
            disks,
            components,
            nvml,
            #[cfg(target_os = "windows")]
            pdh_gpu,
            #[cfg(target_os = "windows")]
            dxgi_adapters,
            sample_count: 0,
        }
    }

    pub fn collect(&mut self) -> HardwareSnapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        let full_enum = self.sample_count % 6 == 0;
        self.disks.refresh(full_enum);
        self.components.refresh(full_enum);
        self.sample_count = self.sample_count.wrapping_add(1);

        let cpus = self.sys.cpus();
        let cpu_brand = cpus
            .first()
            .map(|c| c.brand().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "CPU".to_string());
        let cpu_cores = cpus.len();
        let cpu_usage_percent = self.sys.global_cpu_usage();
        let cpu_freq_mhz = if cpus.is_empty() {
            0
        } else {
            cpus.iter().map(|c| c.frequency()).sum::<u64>() / cpus.len() as u64
        };

        let ram_total_bytes = self.sys.total_memory();
        let ram_used_bytes = self.sys.used_memory();
        let ram_percent = if ram_total_bytes > 0 {
            (ram_used_bytes as f32 / ram_total_bytes as f32) * 100.0
        } else {
            0.0
        };

        let cpu_temp_c = self
            .components
            .iter()
            .filter_map(|c| c.temperature())
            .find(|t| *t > 0.0 && *t < 130.0);

        let disks = self
            .disks
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| {
                let total = d.total_space();
                let avail = d.available_space();
                let used = total.saturating_sub(avail);
                let pct = (used as f32 / total as f32) * 100.0;
                DiskSnapshot {
                    mount: d.mount_point().to_string_lossy().to_string(),
                    name: d.name().to_string_lossy().to_string(),
                    total_bytes: total,
                    available_bytes: avail,
                    used_percent: pct,
                }
            })
            .take(6)
            .collect();

        let gpus = self.collect_gpus();

        HardwareSnapshot {
            cpu_brand,
            cpu_usage_percent,
            cpu_cores,
            cpu_freq_mhz,
            cpu_temp_c,
            ram_used_bytes,
            ram_total_bytes,
            ram_percent,
            gpus,
            disks,
        }
    }

    fn collect_gpus(&mut self) -> Vec<GpuSnapshot> {
        let mut gpus = Vec::new();

        // 1. High-precision NVIDIA NVML telemetry if nvml.dll + NVIDIA driver are available
        if let Some(nvml) = &self.nvml {
            if let Ok(count) = nvml.device_count() {
                for idx in 0..count {
                    if let Ok(dev) = nvml.device_by_index(idx) {
                        let name = dev.name().unwrap_or_else(|_| format!("NVIDIA GPU {idx}"));
                        let util = dev.utilization_rates().ok().map(|u| u.gpu);
                        let mem = dev.memory_info().ok();
                        let temp = dev
                            .temperature(
                                nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu,
                            )
                            .ok();
                        let power = dev.power_usage().ok().map(|mw| mw as f32 / 1000.0);
                        gpus.push(GpuSnapshot {
                            name,
                            utilization_percent: util,
                            vram_used_mb: mem.as_ref().map(|m| m.used / (1024 * 1024)),
                            vram_total_mb: mem.as_ref().map(|m| m.total / (1024 * 1024)),
                            temperature_c: temp,
                            power_watts: power,
                            telemetry_source: "NVML".to_string(),
                        });
                    }
                }
            }
        }

        // 2. Supplement or fallback with cached DXGI adapters + Windows PDH GPU Utilization
        #[cfg(target_os = "windows")]
        {
            let mut pdh_util: Option<Option<u32>> = None;
            for (name, total_vram_mb) in &self.dxgi_adapters {
                let name_lower = name.to_ascii_lowercase();
                let already_listed = gpus.iter().any(|g| {
                    let g_lower = g.name.to_ascii_lowercase();
                    g_lower.contains(&name_lower) || name_lower.contains(&g_lower)
                });
                if !already_listed {
                    let util = *pdh_util
                        .get_or_insert_with(|| self.pdh_gpu.as_mut().and_then(|p| p.sample()));
                    gpus.push(GpuSnapshot {
                        name: name.clone(),
                        utilization_percent: util,
                        vram_used_mb: None,
                        vram_total_mb: Some(*total_vram_mb),
                        temperature_c: None,
                        power_watts: None,
                        telemetry_source: if util.is_some() {
                            "DXGI + PDH".to_string()
                        } else {
                            "DXGI".to_string()
                        },
                    });
                }
            }
        }

        gpus
    }
}

#[cfg(target_os = "windows")]
fn enumerate_dxgi_adapters() -> Vec<(String, u64)> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
    };

    let mut result = Vec::new();
    unsafe {
        let Ok(factory): Result<IDXGIFactory1, _> = CreateDXGIFactory1() else {
            return result;
        };
        let mut idx = 0u32;
        while let Ok(adapter) = factory.EnumAdapters1(idx) {
            idx += 1;
            if let Ok(desc) = adapter.GetDesc1() {
                if (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                    continue;
                }
                let end = desc
                    .Description
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(desc.Description.len());
                let name = String::from_utf16_lossy(&desc.Description[..end]);
                let vram_mb = (desc.DedicatedVideoMemory as u64) / (1024 * 1024);
                if !name.is_empty() {
                    result.push((name, vram_mb));
                }
            }
        }
    }
    result
}

#[cfg(target_os = "windows")]
struct PdhGpuCounter {
    query: isize,
    counter: isize,
}

#[cfg(target_os = "windows")]
impl PdhGpuCounter {
    fn new() -> Option<Self> {
        use windows::core::w;
        use windows::Win32::System::Performance::{
            PdhAddEnglishCounterW, PdhCollectQueryData, PdhOpenQueryW,
        };

        unsafe {
            let mut query = 0isize;
            if PdhOpenQueryW(None, 0, &mut query) != 0 {
                return None;
            }
            let mut counter = 0isize;
            if PdhAddEnglishCounterW(
                query,
                w!("\\GPU Engine(*engtype_3D)\\Utilization Percentage"),
                0,
                &mut counter,
            ) != 0
            {
                let _ = windows::Win32::System::Performance::PdhCloseQuery(query);
                return None;
            }
            let _ = PdhCollectQueryData(query);
            Some(Self { query, counter })
        }
    }

    fn sample(&mut self) -> Option<u32> {
        use windows::Win32::System::Performance::{
            PdhCollectQueryData, PdhGetFormattedCounterValue, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
        };
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return None;
            }
            let mut val = PDH_FMT_COUNTERVALUE::default();
            if PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, None, &mut val) != 0 {
                return None;
            }
            let pct = val.Anonymous.doubleValue.clamp(0.0, 100.0).round() as u32;
            Some(pct)
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for PdhGpuCounter {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::Performance::PdhCloseQuery(self.query);
        }
    }
}
