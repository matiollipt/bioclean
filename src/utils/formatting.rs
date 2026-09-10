pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    const TIB: u64 = GIB * 1024;

    if bytes >= TIB {
        format!("{:.2} TiB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn parse_size_to_bytes(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();
    if s.ends_with("T") || s.ends_with("TB") || s.ends_with("TIB") {
        let num: f64 = s.trim_end_matches(|c: char| c.is_alphabetic()).trim().parse().ok()?;
        Some((num * 1024.0 * 1024.0 * 1024.0 * 1024.0) as u64)
    } else if s.ends_with("G") || s.ends_with("GB") || s.ends_with("GIB") {
        let num: f64 = s.trim_end_matches(|c: char| c.is_alphabetic()).trim().parse().ok()?;
        Some((num * 1024.0 * 1024.0 * 1024.0) as u64)
    } else if s.ends_with("M") || s.ends_with("MB") || s.ends_with("MIB") {
        let num: f64 = s.trim_end_matches(|c: char| c.is_alphabetic()).trim().parse().ok()?;
        Some((num * 1024.0 * 1024.0) as u64)
    } else if s.ends_with("K") || s.ends_with("KB") || s.ends_with("KIB") {
        let num: f64 = s.trim_end_matches(|c: char| c.is_alphabetic()).trim().parse().ok()?;
        Some((num * 1024.0) as u64)
    } else {
        s.parse::<u64>().ok()
    }
}
