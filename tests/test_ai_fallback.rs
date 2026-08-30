use bioclean::ai::fallback::{diagnose_fallback, safety_warning_fallback};
use bioclean::utils::procfs::MemInfo;

#[test]
fn test_diagnose_fallback_score() {
    let mem = MemInfo {
        total_bytes: 32 * 1024 * 1024 * 1024,
        free_bytes: 16 * 1024 * 1024 * 1024,
        available_bytes: 20 * 1024 * 1024 * 1024,
        buffers_bytes: 1024 * 1024,
        cached_bytes: 1024 * 1024,
        swap_total_bytes: 4 * 1024 * 1024 * 1024,
        swap_free_bytes: 4 * 1024 * 1024 * 1024,
    };

    let report = diagnose_fallback(0, 0, 45, &mem, 52.0);
    assert!(report.contains("Health Score & Status"));
    assert!(report.contains("OPTIMAL"));
    assert!(report.contains("100/100"));
}

#[test]
fn test_safety_warning_fallback() {
    let warn_conda = safety_warning_fallback("/home/clever/miniconda3/envs/bio", "delete");
    assert!(warn_conda.contains("Conda/Python environments"));

    let warn_docker = safety_warning_fallback("/var/lib/docker", "prune");
    assert!(warn_docker.contains("Docker images"));
}
