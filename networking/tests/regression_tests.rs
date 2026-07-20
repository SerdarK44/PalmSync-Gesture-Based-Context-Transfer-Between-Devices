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
async fn reg_001_empty_file_name_rejected() {
    let _master = start_master(29301).await;
    let slave = SlaveNode::new("127.0.0.1", 29301, "reg_pass");
    slave.connect().await.unwrap();

    let mut snap = base_snapshot();
    snap.file_name = String::new();
    assert!(slave.send_grab(snap).is_err(), "empty file_name must fail client-side");
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
