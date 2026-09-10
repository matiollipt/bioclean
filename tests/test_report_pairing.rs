use bioclean::modules::diagnose::{collect_diagnostic_snapshot, SystemDiagnosticSnapshot};
use bioclean::modules::history::{HistoryManager, SessionRecord};
use tempfile::tempdir;

#[test]
fn test_run_id_generation_and_uniqueness() {
    let id1 = HistoryManager::generate_run_id("diag");
    let id2 = HistoryManager::generate_run_id("diag");
    assert!(id1.starts_with("diag_"));
    assert!(id2.starts_with("diag_"));
    assert_ne!(id1, id2);
}

#[test]
fn test_paired_session_creation_and_logging() {
    let tmp = tempdir().unwrap();
    let history_file = tmp.path().join("history.json");
    let base_dir = tmp.path().join("sessions");

    let history_mgr = HistoryManager::new(history_file.to_str().unwrap());
    let run_id = HistoryManager::generate_run_id("test_session");

    let session_dir = HistoryManager::create_paired_session_dir(base_dir.to_str().unwrap(), &run_id).unwrap();
    assert!(session_dir.exists());

    // Write audit trail
    assert!(HistoryManager::log_audit_trail(&session_dir, "Session initialization test").is_ok());
    let audit_file = session_dir.join("audit_trail.log");
    assert!(audit_file.exists());
    let audit_content = std::fs::read_to_string(&audit_file).unwrap();
    assert!(audit_content.contains("Session initialization test"));

    // Record session
    let report_file = session_dir.join("report.md");
    let snapshot_file = session_dir.join("diagnostic_snapshot.json");
    std::fs::write(&report_file, "# Test Health Report").unwrap();
    std::fs::write(&snapshot_file, "{}").unwrap();

    let record = SessionRecord {
        session_id: run_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        status: "COMPLETED".to_string(),
        run_type: "DIAGNOSTIC".to_string(),
        report_path: Some(report_file.to_str().unwrap().to_string()),
        snapshot_path: Some(snapshot_file.to_str().unwrap().to_string()),
        rollback_manifest_path: None,
        operations: Vec::new(),
    };
    assert!(history_mgr.record_session(record).is_ok());

    let sessions = history_mgr.load_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, run_id);
    assert_eq!(sessions[0].report_path, Some(report_file.to_str().unwrap().to_string()));
    assert_eq!(sessions[0].snapshot_path, Some(snapshot_file.to_str().unwrap().to_string()));
}

#[test]
fn test_diagnostic_snapshot_serialization() {
    let run_id = "test_snapshot_run";
    let snapshot = collect_diagnostic_snapshot(run_id);
    assert_eq!(snapshot.run_id, run_id);
    assert!(snapshot.cpu.core_count >= 1);

    let json_str = serde_json::to_string_pretty(&snapshot).unwrap();
    assert!(json_str.contains("run_id"));
    assert!(json_str.contains("cpu"));
    assert!(json_str.contains("memory"));
    assert!(json_str.contains("thermals"));

    let deserialized: SystemDiagnosticSnapshot = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.run_id, run_id);
    assert_eq!(deserialized.host.hostname, snapshot.host.hostname);
}
