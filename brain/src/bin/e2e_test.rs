use std::net::UdpSocket;
use std::time::Duration;

use palmsync_networking::{MasterNode, SlaveNode};
use palmsync_shared::{GestureEvent, GesturePacket, StateSnapshot};

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    println!("==============================================");
    println!("  PalmSync E2E Pipeline Test");
    println!("==============================================\n");

    let master_port: u16 = 29001;
    let ipc_port: u16 = 29002;
    let password = "e2e_test_pass";

    // ── 1. Master başlat ──────────────────────────────────────
    println!("[1/7] Master node baslatiliyor...");
    let master = MasterNode::new(&format!("127.0.0.1:{}", master_port), password);
    let master_clone = master.clone();
    tokio::spawn(async move {
        master_clone.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    println!("  [OK] Master dinliyor: 127.0.0.1:{}\n", master_port);

    // ── 2. IPC listener başlat ────────────────────────────────
    println!("[2/7] IPC listener baslatiliyor (UDP:{})...", ipc_port);
    let ipc_socket = tokio::net::UdpSocket::bind(format!("127.0.0.1:{}", ipc_port))
        .await
        .expect("IPC bind failed");
    println!("  [OK] IPC listener hazir\n");

    // ── 3. IPC handler — gesture alır, master state günceller ─
    println!("[3/7] IPC handler gorevi baslatiliyor...");
    let master_for_ipc = master.clone();
    let ipc_handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            let n = ipc_socket.recv(&mut buf).await.expect("UDP recv failed");
            let text = std::str::from_utf8(&buf[..n]).expect("invalid UTF-8");
            let packet: GesturePacket =
                serde_json::from_str(text).expect("invalid gesture JSON");

            println!(
                "  [IPC] Gesture alindi: {:?} (confidence: {:.2})",
                packet.event, packet.confidence
            );

            match packet.event {
                GestureEvent::Grab => {
                    let snap = StateSnapshot {
                        project_path: "C:\\Users\\test\\e2e_project".into(),
                        file_name: "pipeline_test.rs".into(),
                        cursor_line: 42,
                        clipboard_content: "fn e2e_pipeline() { /* full cycle */ }".into(),
                        timestamp: packet.timestamp_ms,
                        source_node_id: "e2e-master-node".into(),
                    };
                    master_for_ipc.set_state(snap);
                    println!("  [IPC] GRAB -> Master state guncellendi");
                }
                GestureEvent::Drop => {
                    let state = master_for_ipc.get_state();
                    match state {
                        Some(s) => println!(
                            "  [IPC] DROP -> State mevcut: {}:{} line {}",
                            s.project_path, s.file_name, s.cursor_line
                        ),
                        None => println!("  [IPC] DROP -> State bos"),
                    }
                }
            }
        }
    });
    println!("  [OK] IPC handler calisiyor\n");

    // ── 4. UDP ile GRAB gesture gonder ────────────────────────
    println!("[4/7] UDP ile GRAB gesture gonderiliyor...");
    let udp_sender = UdpSocket::bind("127.0.0.1:0").expect("sender bind failed");
    let grab_json = r#"{"event":"Grab","timestamp_ms":1700000000000,"confidence":0.97}"#;
    udp_sender
        .send_to(grab_json.as_bytes(), format!("127.0.0.1:{}", ipc_port))
        .expect("UDP send failed");
    println!("  [OK] GRAB paketi gonderildi: {}", grab_json);

    tokio::time::sleep(Duration::from_millis(300)).await;

    // ── 5. Master state dogrula ───────────────────────────────
    println!("\n[5/7] Master state dogrulaniyor...");
    let state = master.get_state();
    assert!(state.is_some(), "Master state GRAB sonrasi bos olmamali!");
    let state = state.unwrap();
    assert_eq!(state.cursor_line, 42);
    assert_eq!(state.file_name, "pipeline_test.rs");
    assert_eq!(state.project_path, "C:\\Users\\test\\e2e_project");
    assert_eq!(
        state.clipboard_content,
        "fn e2e_pipeline() { /* full cycle */ }"
    );
    println!("  [OK] Master state dogrulandi:");
    println!("       Proje   : {}", state.project_path);
    println!("       Dosya   : {}", state.file_name);
    println!("       Satir   : {}", state.cursor_line);
    println!("       Pano    : {}", state.clipboard_content);
    println!();

    // ── 6. Slave baglan + DROP → state al ─────────────────────
    println!("[6/7] Slave baglaniyor ve DROP yapiyor...");
    let slave = SlaveNode::new("127.0.0.1", master_port, password);
    let resp = slave.connect().await.expect("Slave connect failed");
    assert!(resp.accepted, "Slave baglantisi reddedildi!");
    println!("  [OK] Slave baglandi: {}", resp.message);

    let dropped = slave
        .request_drop()
        .await
        .expect("DROP request failed");
    assert!(dropped.is_some(), "DROP sonucu bos olmamali!");
    let dropped = dropped.unwrap();
    assert_eq!(dropped.cursor_line, 42);
    assert_eq!(dropped.file_name, "pipeline_test.rs");
    assert_eq!(dropped.project_path, "C:\\Users\\test\\e2e_project");
    assert_eq!(
        dropped.clipboard_content,
        "fn e2e_pipeline() { /* full cycle */ }"
    );
    println!("  [OK] Slave DROP ile state aldi:");
    println!("       Proje   : {}", dropped.project_path);
    println!("       Dosya   : {}", dropped.file_name);
    println!("       Satir   : {}", dropped.cursor_line);
    println!("       Pano    : {}", dropped.clipboard_content);
    println!();

    // ── 7. UDP ile DROP gesture gonder ────────────────────────
    println!("[7/7] UDP ile DROP gesture gonderiliyor...");
    let drop_json = r#"{"event":"Drop","timestamp_ms":1700000001000,"confidence":0.93}"#;
    udp_sender
        .send_to(drop_json.as_bytes(), format!("127.0.0.1:{}", ipc_port))
        .expect("UDP drop send failed");
    println!("  [OK] DROP paketi gonderildi");

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Cleanup
    ipc_handle.abort();

    println!();
    println!("==============================================");
    println!("  SONUC: E2E Pipeline Testi BASARILI!");
    println!("==============================================");
    println!();
    println!("  Dogrulanan akis:");
    println!("  UDP Gesture -> IPC Listener -> Brain Logic");
    println!("       -> Master RAM State -> WebSocket");
    println!("       -> Slave DROP -> State Dogrulama");
    println!();
    println!("  Tum assert'ler gecti!");
}
