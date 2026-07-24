use homa_lib::model::AgentStatus;
use homa_lib::poller::scan_once;
use std::fs;

#[test]
fn scan_reads_session_files_and_maps_status() {
    let dir = std::env::temp_dir().join(format!("homa-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let pid = std::process::id(); // a live pid so it is not Ended
    let json = format!(
        r#"{{"pid":{pid},"sessionId":"s1","cwd":"C:\\a\\b\\myrepo","name":"n1","status":"idle","startedAt":1,"statusUpdatedAt":2}}"#
    );
    fs::write(dir.join(format!("{pid}.json")), json).unwrap();

    let agents = scan_once(&dir, false);
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].status, AgentStatus::Idle);
    assert_eq!(agents[0].repo, "myrepo");
    fs::remove_dir_all(&dir).unwrap();
}
