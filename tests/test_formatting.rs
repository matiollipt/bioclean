use bioclean::utils::formatting::{format_bytes, format_duration_secs, parse_size_to_bytes};

#[test]
fn test_format_bytes() {
    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(1024), "1.00 KiB");
    assert_eq!(format_bytes(1024 * 1024 * 50), "50.00 MiB");
    assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.00 GiB");
}

#[test]
fn test_parse_size_to_bytes() {
    assert_eq!(parse_size_to_bytes("50M"), Some(50 * 1024 * 1024));
    assert_eq!(parse_size_to_bytes("2G"), Some(2 * 1024 * 1024 * 1024));
    assert_eq!(parse_size_to_bytes("500K"), Some(500 * 1024));
    assert_eq!(parse_size_to_bytes("1024"), Some(1024));
}

#[test]
fn test_format_duration_secs() {
    assert_eq!(format_duration_secs(45), "45s");
    assert_eq!(format_duration_secs(120), "2m 0s");
    assert_eq!(format_duration_secs(3665), "1h 1m 5s");
    assert_eq!(format_duration_secs(90000), "1d 1h 0m");
}
