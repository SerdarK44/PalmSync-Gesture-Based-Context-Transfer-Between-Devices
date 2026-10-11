//! Regression tests — each test pins a specific fixed bug or fragile
//! edge case so it can never silently return.

use std::time::Duration;

use palmsync_networking::{MasterNode, SlaveNode};
use palmsync_shared::validation::Validator;
use palmsync_shared::StateSnapshot;

fn base_snapshot() -> StateSnapshot {
    StateSnapshot {
        project_path: "C:\\Users\\dev\\project".into(),
        file_name: "main.rs".into(),
        cursor_line: 1,
        clipboard_content: "ok".into(),
        timestamp: 1,
        source_node_id: "reg-node".into(),
        browser_url: String::new(),
        file_content: String::new(),
        file_rel_path: String::new(),
        ..Default::default()
    }
}

async fn start_master(port: u16) -> MasterNode {
    let master = MasterNode::new(&format!("127.0.0.1:{}", port), "reg_pass");
    let m = master.clone();
    tokio::spawn(async move {
        let _ = m.start().await;
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    master
}

/// REG-001: Empty file_name must be rejected before hitting the wire.
/// (An empty name would produce a broken `code --goto` invocation on DROP.)
#[tokio::test]
async fn reg_001_contentless_snapshot_rejected() {
    let _master = start_master(29301).await;
    let slave = SlaveNode::new("127.0.0.1", 29301, "reg_pass");
    slave.connect().await.unwrap();

    // A snapshot carrying nothing at all is still rejected...
    let mut empty = base_snapshot();
    empty.file_name = String::new();
    empty.project_path = String::new();
    empty.clipboard_content = String::new();
    empty.browser_url = String::new();
    assert!(
        slave.send_grab(empty).is_err(),
        "contentless snapshot must fail"
    );

    // ...but a browser-tab-only capture (no VS Code file/path) is legitimate.
    // Requiring a file here is what caused "Rejected invalid drop snapshot:
    // path is empty" once browser capture landed.
    let mut browser_only = base_snapshot();
    browser_only.file_name = String::new();
    browser_only.project_path = String::new();
    browser_only.clipboard_content = String::new();
    browser_only.browser_url = "https://example.com/page".into();
    assert!(
        slave.send_grab(browser_only).is_ok(),
        "browser-only capture must be allowed"
    );
}

/// REG-009: a browser-only snapshot must survive the full round trip —
/// master stores it and a DROP delivers it (previously the slave discarded it
/// during validation, reporting "No state available on master").
#[tokio::test]
async fn reg_009_browser_only_snapshot_round_trip() {
    let master = start_master(29309 + 1).await;
    let slave = SlaveNode::new("127.0.0.1", 29310, "reg_pass");
    slave.connect().await.unwrap();

    let mut snap = base_snapshot();
    snap.file_name = String::new();
    snap.project_path = String::new();
    snap.clipboard_content = String::new();
    snap.browser_url = "https://github.com/palmsync".into();
    slave.send_grab(snap).unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        master.get_state().is_some(),
        "master must store a browser-only capture"
    );

    let dropped = slave.request_drop().await.unwrap();
    let dropped = dropped.expect("DROP must deliver the browser-only snapshot");
    assert_eq!(dropped.browser_url, "https://github.com/palmsync");
}

/// REG-010: "transferred file arrives empty". Only the file NAME used to
/// travel, so the other machine opened a new empty file with that name. The
/// snapshot now carries the file contents + relative path; they must survive
/// the encrypted round trip byte-for-byte (incl. a ~1 MB file).
#[tokio::test]
async fn reg_010_file_content_round_trip() {
    let master = start_master(29311).await;
    let slave = SlaveNode::new("127.0.0.1", 29311, "reg_pass");
    slave.connect().await.unwrap();

    let body = format!(
        "// Türkçe ğüşiöç 🚀\n{}",
        "fn main() { println!(\"hi\"); }\n".repeat(30_000)
    );
    let mut snap = base_snapshot();
    snap.file_name = "main.rs".into();
    snap.file_rel_path = "src/main.rs".into();
    snap.file_content = body.clone();
    slave.send_grab(snap).unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;
    let dropped = slave
        .request_drop()
        .await
        .unwrap()
        .expect("DROP must deliver the file");
    assert_eq!(dropped.file_rel_path, "src/main.rs");
    assert_eq!(
        dropped.file_content, body,
        "file contents must arrive intact"
    );
    let _ = master;
}

/// REG-011: problems found on a joined machine must reach the HOST (that's
/// where the person running the session looks). A report sent by the slave
/// is accepted by the master and survives the encrypted link.
#[tokio::test]
async fn reg_011_device_report_reaches_host() {
    let _master = start_master(29312).await;
    let slave = SlaveNode::new("127.0.0.1", 29312, "reg_pass");
    slave.connect().await.unwrap();

    let report = palmsync_shared::DeviceReport {
        machine: "PC-2".into(),
        level: "warn".into(),
        source: "kamera".into(),
        message: "Kamera izni kapalı".into(),
        timestamp: 1,
    };
    slave
        .send_report(report.clone())
        .expect("connected slave can send reports");

    // Invalid reports are refused on the sending side already.
    let mut bad = report;
    bad.level = "nope".into();
    assert!(slave.send_report(bad).is_err());

    // The link must still be healthy afterwards.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(slave.request_drop().await.is_ok());
}

/// REG-002: Disconnected slaves must be removed from the master registry
/// (early versions leaked HashMap entries per reconnect).
#[tokio::test]
async fn reg_002_disconnect_cleans_registry() {
    let master = start_master(29302).await;

    for _ in 0..5 {
        let slave = SlaveNode::new("127.0.0.1", 29302, "reg_pass");
        slave.connect().await.unwrap();
        slave.disconnect();
    }

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        master.connected_slave_count(),
        0,
        "registry must not leak disconnected slaves"
    );
}

/// REG-003: Unicode (Turkish, CJK, emoji) clipboard content must survive
/// the encrypt → wire → decrypt roundtrip byte-for-byte.
#[tokio::test]
async fn reg_003_unicode_clipboard_roundtrip() {
    let master = start_master(29303).await;
    let slave = SlaveNode::new("127.0.0.1", 29303, "reg_pass");
    slave.connect().await.unwrap();

    let unicode = "Merhaba Dünya! 世界 🌍 çğıöşü İĞÜŞÖÇ";
    let mut snap = base_snapshot();
    snap.clipboard_content = unicode.into();
    slave.send_grab(snap).unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;
    let state = master.get_state().unwrap();
    assert_eq!(state.clipboard_content, unicode);
}

/// REG-004: 1 MB clipboard payload through the encrypted channel.
/// (E2EE hex encoding doubles the wire size; must stay under limits.)
#[tokio::test]
async fn reg_004_large_clipboard_through_e2ee() {
    let master = start_master(29304).await;
    let slave = SlaveNode::new("127.0.0.1", 29304, "reg_pass");
    slave.connect().await.unwrap();

    let big = "PalmSync ".repeat(120_000); // ~1 MB
    let mut snap = base_snapshot();
    snap.clipboard_content = big.clone();
    slave.send_grab(snap).unwrap();

    tokio::time::sleep(Duration::from_millis(1000)).await;
    let state = master.get_state().unwrap();
    assert_eq!(state.clipboard_content.len(), big.len());
}

/// REG-005: Empty clipboard is valid (grab with nothing copied).
#[tokio::test]
async fn reg_005_empty_clipboard_is_valid() {
    let mut snap = base_snapshot();
    snap.clipboard_content = String::new();
    assert!(Validator::validate_snapshot(&snap).is_ok());
}

/// REG-006: Slave reconnect after master registry cleanup works
/// (stale outbound channel must be replaced, not reused).
#[tokio::test]
async fn reg_006_slave_reconnect_works() {
    let master = start_master(29306).await;

    let slave = SlaveNode::new("127.0.0.1", 29306, "reg_pass");
    slave.connect().await.unwrap();
    slave.disconnect();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Fresh slave, same master
    let slave2 = SlaveNode::new("127.0.0.1", 29306, "reg_pass");
    slave2.connect().await.unwrap();
    slave2.send_grab(base_snapshot()).unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(master.get_state().is_some());
}

/// REG-007: cursor_line u32::MAX must not panic anywhere in the pipeline
/// (it is rejected by validation, not wrapped or crashed on).
#[tokio::test]
async fn reg_007_extreme_cursor_line_rejected() {
    let mut snap = base_snapshot();
    snap.cursor_line = u32::MAX;
    assert!(Validator::validate_snapshot(&snap).is_err());
}

/// REG-008: short node ids must not panic. The daemon logged `&node_id[..8]`
/// assuming a 36-char UUID, but the UI writes node_id = machine name, which
/// can be < 8 chars — that crashed the daemon before it ever bound, so no
/// master ever listened. `short()` must truncate safely.
#[tokio::test]
async fn reg_008_short_node_id_does_not_panic() {
    assert_eq!(palmsync_shared::short("PC", 8), "PC");
    assert_eq!(palmsync_shared::short("", 8), "");
    assert_eq!(palmsync_shared::short("abcdefghij", 8), "abcdefgh");
    assert_eq!(palmsync_shared::short("çğı", 8), "çğı"); // multi-byte safe

    // A master with a short node id must start and accept a slave.
    let master = MasterNode::new("127.0.0.1:29309", "reg_pass");
    let m = master.clone();
    tokio::spawn(async move {
        let _ = m.start().await;
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    let slave = SlaveNode::new("127.0.0.1", 29309, "reg_pass");
    slave.connect().await.unwrap();
    slave.send_grab(base_snapshot()).unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(master.get_state().is_some());
}
