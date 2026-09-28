use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThermalZone {
    pub id: u32,
    pub zone_type: String,
    pub temp_celsius: f32,
    pub trip_point_crit: Option<f32>,
}

pub fn read_thermal_zones() -> Vec<ThermalZone> {
    let mut zones = Vec::new();
    let thermal_dir = Path::new("/sys/class/thermal");
    if !thermal_dir.exists() {
        return zones;
    }

    if let Ok(entries) = fs::read_dir(thermal_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("thermal_zone") {
                if let Ok(id) = name.trim_start_matches("thermal_zone").parse::<u32>() {
                    let path = entry.path();
                    let zone_type = fs::read_to_string(path.join("type"))
                        .unwrap_or_else(|_| "unknown".to_string())
                        .trim()
                        .to_string();
                    
                    let temp_celsius = fs::read_to_string(path.join("temp"))
                        .ok()
                        .and_then(|s| s.trim().parse::<f32>().ok())
                        .map(|milli| milli / 1000.0)
                        .unwrap_or(0.0);

                    // Check for critical trip point
                    let mut crit_temp = None;
                    for i in 0..10 {
                        let trip_type_file = path.join(format!("trip_point_{}_type", i));
                        let trip_temp_file = path.join(format!("trip_point_{}_temp", i));
                        if trip_type_file.exists() && trip_temp_file.exists() {
                            if let Ok(ttype) = fs::read_to_string(&trip_type_file) {
                                if ttype.trim() == "critical" {
                                    if let Ok(tval) = fs::read_to_string(&trip_temp_file) {
                                        if let Ok(milli) = tval.trim().parse::<f32>() {
                                            crit_temp = Some(milli / 1000.0);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    zones.push(ThermalZone {
                        id,
                        zone_type,
                        temp_celsius,
                        trip_point_crit: crit_temp,
                    });
                }
            }
        }
    }
    zones.sort_by_key(|z| z.id);
    zones
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CpuGovernorInfo {
    pub cpu_id: u32,
    pub current_governor: String,
    pub available_governors: Vec<String>,
    pub energy_perf_preference: Option<String>,
}

pub fn read_cpu_governors() -> Vec<CpuGovernorInfo> {
    let mut cpus = Vec::new();
    let cpu_dir = Path::new("/sys/devices/system/cpu");
    if !cpu_dir.exists() {
        return cpus;
    }

    if let Ok(entries) = fs::read_dir(cpu_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) {
                if let Ok(cpu_id) = name[3..].parse::<u32>() {
                    let cpufreq_dir = entry.path().join("cpufreq");
                    if cpufreq_dir.exists() {
                        let current_governor = fs::read_to_string(cpufreq_dir.join("scaling_governor"))
                            .unwrap_or_else(|_| "unknown".to_string())
                            .trim()
                            .to_string();
                        
                        let available_governors = fs::read_to_string(cpufreq_dir.join("scaling_available_governors"))
                            .unwrap_or_default()
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect();

                        let energy_perf_preference = fs::read_to_string(cpufreq_dir.join("energy_performance_preference"))
                            .ok()
                            .map(|s| s.trim().to_string());

                        cpus.push(CpuGovernorInfo {
                            cpu_id,
                            current_governor,
                            available_governors,
                            energy_perf_preference,
                        });
                    }
                }
            }
        }
    }
    cpus.sort_by_key(|c| c.cpu_id);
    cpus
}

pub fn set_cpu_governors(governor: &str) -> Result<usize> {
    let mut modified = 0;
    let cpu_dir = Path::new("/sys/devices/system/cpu");
    if let Ok(entries) = fs::read_dir(cpu_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) {
                let gov_file = entry.path().join("cpufreq/scaling_governor");
                if gov_file.exists() {
                    if fs::write(&gov_file, governor.as_bytes()).is_ok() {
                        modified += 1;
                    }
                }
                // Also try energy_performance_preference if applicable
                let epp_file = entry.path().join("cpufreq/energy_performance_preference");
                if epp_file.exists() {
                    let epp_val = match governor {
                        "performance" => "performance",
                        "powersave" => "power",
                        _ => "balance_performance",
                    };
                    let _ = fs::write(&epp_file, epp_val.as_bytes());
                }
            }
        }
    }
    Ok(modified)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SocketEntry {
    pub protocol: String,
    pub local_addr: String,
    pub local_port: u16,
    pub remote_addr: String,
    pub remote_port: u16,
    pub state: String,
    pub inode: u64,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

fn parse_hex_v4(hex_addr: &str) -> Option<(String, u16)> {
    let parts: Vec<&str> = hex_addr.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let ip_u32 = u32::from_str_radix(parts[0], 16).ok()?;
    let ip = Ipv4Addr::from(ip_u32.to_ne_bytes());
    let port = u16::from_str_radix(parts[1], 16).ok()?;
    Some((ip.to_string(), port))
}

fn parse_hex_v6(hex_addr: &str) -> Option<(String, u16)> {
    let parts: Vec<&str> = hex_addr.split(':').collect();
    if parts.len() != 2 || parts[0].len() != 32 {
        return None;
    }
    let mut octets = [0u8; 16];
    for (i, chunk) in parts[0].as_bytes().chunks(8).enumerate() {
        let chunk_str = std::str::from_utf8(chunk).ok()?;
        let word = u32::from_str_radix(chunk_str, 16).ok()?;
        let bytes = word.to_ne_bytes();
        octets[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
    }
    let ip = Ipv6Addr::from(octets);
    let port = u16::from_str_radix(parts[1], 16).ok()?;
    Some((ip.to_string(), port))
}

fn map_tcp_state(st: &str) -> String {
    match st {
        "01" => "ESTABLISHED".to_string(),
        "02" => "SYN_SENT".to_string(),
        "03" => "SYN_RECV".to_string(),
        "04" => "FIN_WAIT1".to_string(),
        "05" => "FIN_WAIT2".to_string(),
        "06" => "TIME_WAIT".to_string(),
        "07" => "CLOSE".to_string(),
        "08" => "CLOSE_WAIT".to_string(),
        "09" => "LAST_ACK".to_string(),
        "0A" => "LISTEN".to_string(),
        "0B" => "CLOSING".to_string(),
        _ => st.to_string(),
    }
}

fn build_inode_to_pid_map() -> HashMap<u64, (u32, String)> {
    let mut map = HashMap::new();
    let proc_dir = Path::new("/proc");
    if let Ok(entries) = fs::read_dir(proc_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Ok(pid) = name.parse::<u32>() {
                let fd_dir = entry.path().join("fd");
                let proc_name = fs::read_to_string(entry.path().join("comm"))
                    .unwrap_or_else(|_| "unknown".to_string())
                    .trim()
                    .to_string();

                if let Ok(fd_entries) = fs::read_dir(fd_dir) {
                    for fd_entry in fd_entries.flatten() {
                        if let Ok(link) = fs::read_link(fd_entry.path()) {
                            let link_str = link.to_string_lossy();
                            if link_str.starts_with("socket:[") && link_str.ends_with(']') {
                                let inode_str = &link_str[8..link_str.len() - 1];
                                if let Ok(inode) = inode_str.parse::<u64>() {
                                    map.insert(inode, (pid, proc_name.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    map
}

pub fn read_active_sockets() -> Vec<SocketEntry> {
    let mut sockets = Vec::new();
    let inode_map = build_inode_to_pid_map();

    let sources = [
        ("/proc/net/tcp", "TCP", false),
        ("/proc/net/tcp6", "TCP6", true),
        ("/proc/net/udp", "UDP", false),
        ("/proc/net/udp6", "UDP6", true),
    ];

    for (file_path, proto, is_v6) in sources {
        if let Ok(content) = fs::read_to_string(file_path) {
            for line in content.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 9 {
                    let local = if is_v6 {
                        parse_hex_v6(parts[1])
                    } else {
                        parse_hex_v4(parts[1])
                    };
                    let remote = if is_v6 {
                        parse_hex_v6(parts[2])
                    } else {
                        parse_hex_v4(parts[2])
                    };
                    let state = map_tcp_state(parts[3]);
                    let inode = parts[9].parse::<u64>().unwrap_or(0);

                    if let (Some((l_addr, l_port)), Some((r_addr, r_port))) = (local, remote) {
                        let (pid, process_name) = if let Some((p, n)) = inode_map.get(&inode) {
                            (Some(*p), Some(n.clone()))
                        } else {
                            (None, None)
                        };

                        sockets.push(SocketEntry {
                            protocol: proto.to_string(),
                            local_addr: l_addr,
                            local_port: l_port,
                            remote_addr: r_addr,
                            remote_port: r_port,
                            state,
                            inode,
                            pid,
                            process_name,
                        });
                    }
                }
            }
        }
    }
    sockets
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MemInfo {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub buffers_bytes: u64,
    pub cached_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_free_bytes: u64,
}

pub fn read_meminfo() -> MemInfo {
    let mut info = MemInfo::default();
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(key), Some(val_str)) = (parts.next(), parts.next()) {
                if let Ok(kb) = val_str.parse::<u64>() {
                    let bytes = kb * 1024;
                    match key {
                        "MemTotal:" => info.total_bytes = bytes,
                        "MemFree:" => info.free_bytes = bytes,
                        "MemAvailable:" => info.available_bytes = bytes,
                        "Buffers:" => info.buffers_bytes = bytes,
                        "Cached:" => info.cached_bytes = bytes,
                        "SwapTotal:" => info.swap_total_bytes = bytes,
                        "SwapFree:" => info.swap_free_bytes = bytes,
                        _ => {}
                    }
                }
            }
        }
    }
    info
}

#[derive(Debug, Clone, Copy)]
pub enum PowerSource {
    Battery,
    Rapl,
}

#[derive(Debug, Clone, Copy)]
pub struct PowerReading {
    pub watts: f32,
    pub source: PowerSource,
    /// Instantaneous battery voltage in volts, when the source is `Battery`.
    /// RAPL has no notion of voltage, so this is always `None` for `Rapl`.
    pub voltage: Option<f32>,
}

fn read_battery_power_now() -> Option<(f32, Option<f32>)> {
    // /sys/class/power_supply/BAT*/power_now and voltage_now are in
    // microwatts/microvolts, instantaneous.
    for entry in fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("BAT") {
            if let Ok(s) = fs::read_to_string(entry.path().join("power_now")) {
                if let Ok(uw) = s.trim().parse::<f32>() {
                    let voltage = fs::read_to_string(entry.path().join("voltage_now"))
                        .ok()
                        .and_then(|s| s.trim().parse::<f32>().ok())
                        .map(|uv| uv / 1_000_000.0);
                    return Some((uw / 1_000_000.0, voltage));
                }
            }
        }
    }
    None
}

/// Reads a power draw sample, trying battery `power_now` first (instantaneous,
/// laptops only) then falling back to RAPL `energy_uj` (desktop/workstation,
/// requires a delta between two samples over time). Returns `None` for the
/// reading when unavailable on this system (most desktops without a battery
/// or RAPL support) — this is an expected, non-error outcome, never a panic.
/// The second element of the tuple is the new `(Instant, energy_uj)` sample to
/// pass back in on the next call for the RAPL delta; `None` when not using RAPL.
const RAPL_ENERGY_PATH: &str = "/sys/class/powercap/intel-rapl:0/energy_uj";

/// True if this process was denied read access to the RAPL energy counter
/// that exists on this system (common: root-only permissions), as opposed to
/// the sensor simply not existing (no RAPL support, no battery).
pub fn power_reading_permission_denied() -> bool {
    Path::new(RAPL_ENERGY_PATH).exists() && fs::read_to_string(RAPL_ENERGY_PATH).is_err()
}

pub fn read_power_reading(prev: Option<(Instant, u64)>) -> (Option<PowerReading>, Option<(Instant, u64)>) {
    if let Some((watts, voltage)) = read_battery_power_now() {
        return (Some(PowerReading { watts, source: PowerSource::Battery, voltage }), None);
    }

    if let Ok(s) = fs::read_to_string(RAPL_ENERGY_PATH) {
        if let Ok(energy_uj) = s.trim().parse::<u64>() {
            let now = Instant::now();
            if let Some((prev_t, prev_e)) = prev {
                let dt = now.duration_since(prev_t).as_secs_f32();
                if dt > 0.0 {
                    let watts = (energy_uj.saturating_sub(prev_e) as f32 / 1_000_000.0) / dt;
                    return (Some(PowerReading { watts, source: PowerSource::Rapl, voltage: None }), Some((now, energy_uj)));
                }
            }
            return (None, Some((now, energy_uj))); // first sample, no delta yet
        }
    }

    (None, None) // genuinely unavailable on this system (no battery, no RAPL, or RAPL unreadable)
}

/// Extracts journal disk usage in bytes via `journalctl --disk-usage`.
/// Shared by the telemetry panel and `free::estimate_reclaimable()` so the
/// parsing logic lives in one place.
pub fn read_journal_size() -> Option<u64> {
    if !crate::utils::system::command_exists("journalctl") {
        return None;
    }
    let out = crate::utils::system::run_cmd_stdout("journalctl", &["--disk-usage"]).ok()?;
    let re = regex::Regex::new(r"([0-9.]+)([KMGT]B?)").ok()?;
    let caps = re.captures(&out)?;
    let num_str = caps.get(1)?.as_str();
    let unit_str = caps.get(2)?.as_str();
    let combined = format!("{}{}", num_str, unit_str);
    crate::utils::formatting::parse_size_to_bytes(&combined)
}

pub fn read_loadavg() -> (f64, f64, f64) {
    if let Ok(content) = fs::read_to_string("/proc/loadavg") {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() >= 3 {
            let l1 = parts[0].parse::<f64>().unwrap_or(0.0);
            let l5 = parts[1].parse::<f64>().unwrap_or(0.0);
            let l15 = parts[2].parse::<f64>().unwrap_or(0.0);
            return (l1, l5, l15);
        }
    }
    (0.0, 0.0, 0.0)
}
