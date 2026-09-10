use sysinfo::Disks;

#[derive(Debug, Clone)]
pub struct DiskUsageInfo {
    pub mount_point: String,
    pub kind: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

impl DiskUsageInfo {
    pub fn used_percent(&self) -> u32 {
        if self.total_bytes == 0 {
            0
        } else {
            ((self.used_bytes as f64 / self.total_bytes as f64) * 100.0) as u32
        }
    }
}

/// Enumerates internal (non-removable, non-media-mount) disks using `sysinfo`,
/// mirroring `hdd.rs`'s external-mount scan but with the filter inverted.
pub fn read_internal_disks() -> Vec<DiskUsageInfo> {
    Disks::new_with_refreshed_list()
        .iter()
        .filter(|d| {
            let mp = d.mount_point().to_string_lossy();
            !(mp.starts_with("/media")
                || mp.starts_with("/mnt")
                || mp.starts_with("/run/media")
                || mp.starts_with("/boot")
                || mp.starts_with("/snap")
                || d.is_removable())
        })
        .map(|d| DiskUsageInfo {
            mount_point: d.mount_point().to_string_lossy().into_owned(),
            kind: format!("{:?}", d.kind()),
            total_bytes: d.total_space(),
            used_bytes: d.total_space().saturating_sub(d.available_space()),
            available_bytes: d.available_space(),
        })
        .collect()
}
