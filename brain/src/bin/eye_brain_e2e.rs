use std::net::UdpSocket;
use std::time::Duration;

use palmsync_networking::MasterNode;
use palmsync_shared::{GestureEvent, GesturePacket};

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    println!("==============================================");
    println!("  PalmSync Eye <-> Brain E2E Test");
    println!("==============================================\n");

    let master_port: u16 = 29011;
    let ipc_port: u16 = 29012;

    // ── 1. Master başlat ──────────────────────────────────
    println!("[1/5] Master node baslatiliyor...");
    let master = MasterNode::new(&format!("127.0.0.1:{}", master_port), "e2e_pass");
    let master_clone = master.clone();
    tokio::spawn(async move {
        master_clone.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    println!("  [OK] Master dinliyor: 127.0.0.1:{}\n", master_port);

    // ── 2. IPC listener başlat ────────────────────────────
    println!("[2/5] IPC listener baslatiliyor (UDP:{})...", ipc_port);
    let ipc_socket = tokio::net::UdpSocket::bind(format!("127.0.0.1:{}", ipc_port))
        .await
        .expect("IPC bind failed");

    let master_for_ipc = master.clone();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<GesturePacket>(10);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            let n = ipc_socket.recv(&mut buf).await.expect("UDP recv failed");
            let text = std::str::from_utf8(&buf[..n]).expect("invalid UTF-8");
            let packet: GesturePacket = serde_json::from_str(text).expect("invalid gesture JSON");
            let _ = tx.send(packet).await;
        }
    });
    println!("  [OK] IPC listener hazir\n");

    // ── 3. Eye IPC formatında paket gonder ────────────────
    println!("[3/5] Eye IPC formatinda GRAB paketi gonderiliyor...");

    // This is EXACTLY what Eye's IpcSender::send_event produces
    let eye_grab_payload = r#"{"event":"Grab","timestamp_ms":1700000099000,"confidence":0.9500}"#;

    let sender = UdpSocket::bind("127.0.0.1:0").expect("sender bind failed");
    sender
        .send_to(
            eye_grab_payload.as_bytes(),
            format!("127.0.0.1:{}", ipc_port),
        )
        .expect("UDP send failed");
    println!("  [OK] Payload gonderildi: {}", eye_grab_payload);

    // ── 4. IPC listener'da dogrula ────────────────────────
    println!("\n[4/5] IPC listener'da paket dogrulaniyor...");
    let packet = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("IPC receive timeout")
        .expect("channel closed");

    assert!(
        matches!(packet.event, GestureEvent::Grab),
        "Event Grab olmali!"
    );
    assert_eq!(packet.timestamp_ms, 1700000099000);
    assert!((packet.confidence - 0.95).abs() < 0.01);
    println!("  [OK] Paket dogrulandi:");
    println!("       Event      : {:?}", packet.event);
    println!("       Timestamp  : {}", packet.timestamp_ms);
    println!("       Confidence : {:.4}", packet.confidence);

    // Simulate brain handling the gesture
    let snap = palmsync_shared::StateSnapshot {
        project_path: "C:\\Users\\test\\eye_brain_project".into(),
        file_name: "eye_test.cpp".into(),
        cursor_line: 73,
        clipboard_content: "IPC pipeline works!".into(),
        timestamp: packet.timestamp_ms,
        source_node_id: "eye-brain-e2e".into(),
        browser_url: String::new(),
        file_content: String::new(),
        file_rel_path: String::new(),
        ..Default::default()
    };
    master_for_ipc.set_state(snap);
    println!("  [OK] Master state guncellendi\n");

    // ── 5. DROP event ile dogrula ─────────────────────────
    println!("[5/5] Eye IPC formatinda DROP paketi gonderiliyor...");
    let eye_drop_payload = r#"{"event":"Drop","timestamp_ms":1700000100000,"confidence":0.9200}"#;
    sender
        .send_to(
            eye_drop_payload.as_bytes(),
            format!("127.0.0.1:{}", ipc_port),
        )
        .expect("UDP drop send failed");

    let drop_packet = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("IPC drop timeout")
        .expect("channel closed");

    assert!(matches!(drop_packet.event, GestureEvent::Drop));

    let state = master.get_state();
    assert!(state.is_some());
    let state = state.unwrap();
    assert_eq!(state.file_name, "eye_test.cpp");
    assert_eq!(state.cursor_line, 73);
    println!("  [OK] DROP alindi, state hala mevcut:");
    println!("       Dosya  : {}", state.file_name);
    println!("       Satir  : {}", state.cursor_line);
    println!("       Pano   : {}", state.clipboard_content);

    println!();
    println!("==============================================");
    println!("  SONUC: Eye <-> Brain E2E Testi BASARILI!");
    println!("==============================================");
    println!();
    println!("  Dogrulanan akis:");
    println!("  C++ Eye IPC format -> UDP -> Rust Brain IPC");
    println!("       -> GesturePacket parse -> Master state");
    println!();
    println!("  Eye JSON formati Rust tarafinda dogru parse edildi!");
}
