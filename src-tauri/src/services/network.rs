use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSnapshot {
    pub connected: bool,
    pub adapter_name: String,
    pub download_bps: f64,
    pub upload_bps: f64,
    pub ping_ms: Option<u32>,
    pub vpn_active: bool,
    pub vpn_name: Option<String>,
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
}

pub struct NetworkMonitor {
    last_rx: u64,
    last_tx: u64,
    last_sample: Option<Instant>,
    last_ping: Option<u32>,
    last_ping_check: Option<Instant>,
}

impl Default for NetworkMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkMonitor {
    pub fn new() -> Self {
        Self {
            last_rx: 0,
            last_tx: 0,
            last_sample: None,
            last_ping: None,
            last_ping_check: None,
        }
    }

    pub fn collect(&mut self) -> NetworkSnapshot {
        let (connected, adapter_name, rx, tx, vpn_active, vpn_name) = query_interfaces();
        let now = Instant::now();

        let (download_bps, upload_bps) = match self.last_sample {
            Some(prev) => {
                let dt = now.duration_since(prev).as_secs_f64();
                if dt > 0.1 {
                    let drx = rx.saturating_sub(self.last_rx) as f64 / dt;
                    let dtx = tx.saturating_sub(self.last_tx) as f64 / dt;
                    (drx, dtx)
                } else {
                    (0.0, 0.0)
                }
            }
            None => (0.0, 0.0),
        };

        self.last_rx = rx;
        self.last_tx = tx;
        self.last_sample = Some(now);

        // Ping at most once every 6 seconds when connected to keep overhead near zero
        if connected {
            let due = self
                .last_ping_check
                .is_none_or(|t| now.duration_since(t).as_secs() >= 6);
            if due {
                self.last_ping = measure_icmp_ping();
                self.last_ping_check = Some(now);
            }
        } else {
            self.last_ping = None;
        }

        NetworkSnapshot {
            connected,
            adapter_name,
            download_bps,
            upload_bps,
            ping_ms: self.last_ping,
            vpn_active,
            vpn_name,
            total_rx_bytes: rx,
            total_tx_bytes: tx,
        }
    }
}

#[cfg(target_os = "windows")]
fn query_interfaces() -> (bool, String, u64, u64, bool, Option<String>) {
    use windows::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;

    let mut total_rx = 0u64;
    let mut total_tx = 0u64;
    let mut connected = false;
    let mut primary_adapter = String::from("Disconnected");
    let mut best_rx = 0u64;
    let mut vpn_active = false;
    let mut vpn_name = None;

    const VPN_KEYWORDS: &[&str] = &[
        "wireguard",
        "wiresock",
        "wintun",
        "openvpn",
        "tap-windows",
        "tailscale",
        "nordlynx",
        "mullvad",
        "protonvpn",
        "zerotier",
        "cisco anyconnect",
        "fortinet",
        "globalprotect",
        "tunnel",
        "vpn",
    ];

    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return (false, primary_adapter, 0, 0, false, None);
        }

        let count = (*table).NumEntries as usize;
        let rows: &[MIB_IF_ROW2] = std::slice::from_raw_parts((*table).Table.as_ptr(), count);

        for row in rows {
            // Skip software loopback (24) and disconnected interfaces
            if row.Type == 24 || row.OperStatus != IfOperStatusUp {
                continue;
            }
            // Skip non-hardware filter/miniport duplicates when InterfaceAndOperStatusFlags.FilterInterface is true
            if row.InterfaceAndOperStatusFlags._bitfield & 0b10 != 0 {
                continue;
            }

            let alias = utf16_z_to_string(&row.Alias);
            let desc = utf16_z_to_string(&row.Description);
            let combined_lower = format!(
                "{} {}",
                alias.to_ascii_lowercase(),
                desc.to_ascii_lowercase()
            );

            let is_vpn = VPN_KEYWORDS.iter().any(|k| combined_lower.contains(k));
            if is_vpn {
                vpn_active = true;
                if vpn_name.is_none() {
                    vpn_name = Some(if !alias.is_empty() {
                        alias.clone()
                    } else {
                        desc.clone()
                    });
                }
            }

            if row.InOctets > 0 || row.OutOctets > 0 {
                connected = true;
                total_rx = total_rx.saturating_add(row.InOctets);
                total_tx = total_tx.saturating_add(row.OutOctets);
                if row.InOctets >= best_rx {
                    best_rx = row.InOctets;
                    primary_adapter = if !alias.is_empty() { alias } else { desc };
                }
            }
        }

        FreeMibTable(table as *const _);
    }

    (
        connected,
        primary_adapter,
        total_rx,
        total_tx,
        vpn_active,
        vpn_name,
    )
}

#[cfg(target_os = "windows")]
fn utf16_z_to_string(slice: &[u16]) -> String {
    let len = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..len]).trim().to_string()
}

#[cfg(target_os = "windows")]
fn measure_icmp_ping() -> Option<u32> {
    use windows::Win32::NetworkManagement::IpHelper::{
        IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY,
    };

    unsafe {
        let handle = IcmpCreateFile().ok()?;
        if handle.is_invalid() {
            return None;
        }
        // 1.1.1.1 in network byte order (little endian representation on x86: 0x01010101)
        let dest_addr: u32 = u32::from_ne_bytes([1, 1, 1, 1]);
        let send_data = b"pillvyn";
        let mut reply_buf = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + 64];

        let count = IcmpSendEcho(
            handle,
            dest_addr,
            send_data.as_ptr() as *const _,
            send_data.len() as u16,
            None,
            reply_buf.as_mut_ptr() as *mut _,
            reply_buf.len() as u32,
            1200,
        );

        let _ = IcmpCloseHandle(handle);

        if count > 0 {
            let reply = &*(reply_buf.as_ptr() as *const ICMP_ECHO_REPLY);
            if reply.Status == 0 {
                return Some(reply.RoundTripTime);
            }
        }
        None
    }
}

#[cfg(not(target_os = "windows"))]
fn query_interfaces() -> (bool, String, u64, u64, bool, Option<String>) {
    (false, "Unsupported OS".into(), 0, 0, false, None)
}

#[cfg(not(target_os = "windows"))]
fn measure_icmp_ping() -> Option<u32> {
    None
}
