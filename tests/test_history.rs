use bioclean::modules::history::HistoryManager;
use tempfile::NamedTempFile;

#[test]
fn test_history_session_lifecycle() {
    let tmp = NamedTempFile::new().unwrap();
    let mgr = HistoryManager::new(tmp.path().to_str().unwrap());

    // Initially empty
    let sessions = mgr.load_sessions().unwrap();
    assert_eq!(sessions.len(), 0);

    // Start session
    let sid = mgr.start_session().unwrap();
    assert!(sid.starts_with("session_"));

    // Record op
    mgr.record_op(&sid, "MOVE_AND_SYMLINK", "/mock/src", "/mock/dest", 1024 * 1024).unwrap();

    let updated = mgr.load_sessions().unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].operations.len(), 1);
    assert_eq!(updated[0].operations[0].src, "/mock/src");
    assert_eq!(updated[0].operations[0].dest, "/mock/dest");
}
