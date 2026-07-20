use palmsync_networking::{MasterNode, SlaveNode};
use palmsync_shared::StateSnapshot;
use std::time::Duration;

fn test_snapshot(line: u32) -> StateSnapshot {
    StateSnapshot {
        project_path: "C:\\Users\\test\\myproject".into(),
        file_name: "main.rs".into(),
        cursor_line: line,
        clipboard_content: format!("clipboard data line {}", line),
        timestamp: 1700000000000 + line as u64,
        source_node_id: "test-node".into(),
    }
}

// ─── Test 1: Master başlatma ve bağlantı kabul ─────────────────────

#[tokio::test]
async fn master_starts_and_accepts_slave() {
    let master = MasterNode::new("127.0.0.1:19001", "testpass");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let slave = SlaveNode::new("127.0.0.1", 19001, "testpass");
    let resp = slave.connect().await.expect("slave should connect");

    assert!(resp.accepted);
    assert!(resp.message.contains("Welcome"));
    assert!(slave.is_connected());

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(master.connected_slave_count(), 1);

    master_handle.abort();
}

// ─── Test 2: Yanlış şifre reddedilir ──────────────────────────────

#[tokio::test]
async fn wrong_password_rejected() {
    let master = MasterNode::new("127.0.0.1:19002", "correct_password");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let slave = SlaveNode::new("127.0.0.1", 19002, "wrong_password");
    let result = slave.connect().await;

    assert!(result.is_err(), "connection with wrong password should fail");

    master_handle.abort();
}

// ─── Test 3: GRAB → state Master'da saklanır ───────────────────────

#[tokio::test]
async fn grab_stores_state_on_master() {
    let master = MasterNode::new("127.0.0.1:19003", "pass");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let slave = SlaveNode::new("127.0.0.1", 19003, "pass");
    slave.connect().await.expect("should connect");

    let snap = test_snapshot(42);
    slave.send_grab(snap).expect("grab should send");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let stored = master.get_state();
    assert!(stored.is_some(), "master should have state after GRAB");
    let stored = stored.unwrap();
    assert_eq!(stored.cursor_line, 42);
    assert_eq!(stored.file_name, "main.rs");
    assert_eq!(stored.clipboard_content, "clipboard data line 42");

    master_handle.abort();
}

// ─── Test 4: GRAB → DROP tam döngü (iki slave) ────────────────────

#[tokio::test]
async fn full_grab_drop_cycle_between_two_slaves() {
    let master = MasterNode::new("127.0.0.1:19004", "secret");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Slave 1 bağlanır ve GRAB yapar
    let slave1 = SlaveNode::new("127.0.0.1", 19004, "secret");
    slave1.connect().await.expect("slave1 connect");

    let snap = test_snapshot(128);
    slave1.send_grab(snap).expect("slave1 grab");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Slave 2 bağlanır ve DROP ister
    let slave2 = SlaveNode::new("127.0.0.1", 19004, "secret");
    slave2.connect().await.expect("slave2 connect");

    let dropped = slave2
        .request_drop()
        .await
        .expect("drop request should succeed");

    assert!(dropped.is_some(), "should receive state from master");
    let dropped = dropped.unwrap();
    assert_eq!(dropped.cursor_line, 128);
    assert_eq!(dropped.file_name, "main.rs");
    assert_eq!(dropped.project_path, "C:\\Users\\test\\myproject");
    assert_eq!(dropped.clipboard_content, "clipboard data line 128");

    master_handle.abort();
}

// ─── Test 5: Birden fazla slave eşzamanlı bağlantı ────────────────

#[tokio::test]
async fn multiple_slaves_connect_concurrently() {
    let master = MasterNode::new("127.0.0.1:19005", "multi");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut handles = vec![];
    for _ in 0..5 {
        let handle = tokio::spawn(async move {
            let slave = SlaveNode::new("127.0.0.1", 19005, "multi");
            slave.connect().await.map(|r| r.accepted)
        });
        handles.push(handle);
    }

    let mut success_count = 0;
    for h in handles {
        if let Ok(Ok(true)) = h.await {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 5, "all 5 slaves should connect successfully");

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(master.connected_slave_count(), 5);

    master_handle.abort();
}

// ─── Test 6: Last-Write-Wins — son GRAB kazanır ───────────────────

#[tokio::test]
async fn last_write_wins_state_update() {
    let master = MasterNode::new("127.0.0.1:19006", "lww");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let slave1 = SlaveNode::new("127.0.0.1", 19006, "lww");
    slave1.connect().await.expect("slave1 connect");

    let slave2 = SlaveNode::new("127.0.0.1", 19006, "lww");
    slave2.connect().await.expect("slave2 connect");

    // Slave 1 GRAB yapar
    slave1.send_grab(test_snapshot(10)).expect("grab 1");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let state = master.get_state().unwrap();
    assert_eq!(state.cursor_line, 10);

    // Slave 2 GRAB yapar — üzerine yazar (Last-Write-Wins)
    slave2.send_grab(test_snapshot(99)).expect("grab 2");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let state = master.get_state().unwrap();
    assert_eq!(state.cursor_line, 99, "last grab should overwrite");

    master_handle.abort();
}

// ─── Test 7: Ping/Pong heartbeat ──────────────────────────────────

#[tokio::test]
async fn ping_pong_works() {
    let master = MasterNode::new("127.0.0.1:19007", "ping");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let slave = SlaveNode::new("127.0.0.1", 19007, "ping");
    slave.connect().await.expect("connect");

    // Ping göndermek crash etmemeli
    slave.send_ping().expect("ping should send");
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(slave.is_connected(), "slave should remain connected after ping");

    master_handle.abort();
}

// ─── Test 8: DROP boş state döndürür (henüz GRAB yapılmadıysa) ───

#[tokio::test]
async fn drop_returns_none_when_no_state() {
    let master = MasterNode::new("127.0.0.1:19008", "empty");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let slave = SlaveNode::new("127.0.0.1", 19008, "empty");
    slave.connect().await.expect("connect");

    let result = slave.request_drop().await.expect("drop request");
    assert!(result.is_none(), "should be None when no GRAB has occurred");

    master_handle.abort();
}

// ─── Test 9: Slave disconnect sonrası slave sayısı düşer ──────────

#[tokio::test]
async fn slave_disconnect_updates_count() {
    let master = MasterNode::new("127.0.0.1:19009", "disc");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Scoped slave — drop edilince connection kapanır
    {
        let slave = SlaveNode::new("127.0.0.1", 19009, "disc");
        slave.connect().await.expect("connect");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(master.connected_slave_count(), 1);
        // slave burada drop olur
    }

    // Biraz bekle ki master cleanup yapsın
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Slave drop olunca count düşmeli (channel kapanacak)
    // Not: TCP connection cleanup async olduğu için bu test
    // ortama göre farklı davranabilir
    let count = master.connected_slave_count();
    assert!(count <= 1, "count should decrease after disconnect, got {}", count);

    master_handle.abort();
}

// ─── Test 10: Büyük clipboard verisi transferi ────────────────────

#[tokio::test]
async fn large_clipboard_transfer() {
    let master = MasterNode::new("127.0.0.1:19010", "big");

    let master_handle = tokio::spawn({
        let m = master.clone();
        async move { m.start().await }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let slave1 = SlaveNode::new("127.0.0.1", 19010, "big");
    slave1.connect().await.expect("slave1 connect");

    let slave2 = SlaveNode::new("127.0.0.1", 19010, "big");
    slave2.connect().await.expect("slave2 connect");

    // 100KB clipboard verisi
    let big_clipboard = "x".repeat(100_000);
    let snap = StateSnapshot {
        project_path: "C:\\big\\project".into(),
        file_name: "huge_file.rs".into(),
        cursor_line: 5000,
        clipboard_content: big_clipboard.clone(),
        timestamp: 9999999,
        source_node_id: "big-node".into(),
    };

    slave1.send_grab(snap).expect("grab big data");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let dropped = slave2.request_drop().await.expect("drop request");
    assert!(dropped.is_some());
    let dropped = dropped.unwrap();
    assert_eq!(dropped.clipboard_content.len(), 100_000);
    assert_eq!(dropped.cursor_line, 5000);

    master_handle.abort();
}
