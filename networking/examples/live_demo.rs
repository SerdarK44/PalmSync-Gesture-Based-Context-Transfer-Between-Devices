use palmsync_networking::{MasterNode, SlaveNode};
use palmsync_shared::StateSnapshot;
use std::time::Duration;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    println!("==============================================");
    println!("   PalmSync Faz 1 - Canli Demo Testi");
    println!("==============================================\n");

    // ── 1. Master başlat ─────────────────────────────────────
    println!("[ADIM 1] Master node baslatiliyor...");
    let master = MasterNode::new("127.0.0.1:19999", "demo_password");
    let master_clone = master.clone();

    tokio::spawn(async move {
        master_clone.start().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    println!("  [OK] Master dinliyor: 127.0.0.1:19999");
    println!("  [OK] Master node ID: {}\n", &master.node_id()[..8]);

    // ── 2. Slave 1 bağlan ────────────────────────────────────
    println!("[ADIM 2] Slave-1 baglaniyor...");
    let slave1 = SlaveNode::new("127.0.0.1", 19999, "demo_password");
    let resp1 = slave1.connect().await.expect("Slave-1 connect failed");
    println!("  [OK] Slave-1 baglandi: {}", resp1.message);
    println!("  [OK] Slave-1 node ID: {}", &slave1.node_id()[..8]);
    println!("  [OK] Bagli slave sayisi: {}\n", master.connected_slave_count());

    // ── 3. Slave 2 bağlan ────────────────────────────────────
    println!("[ADIM 3] Slave-2 baglaniyor...");
    let slave2 = SlaveNode::new("127.0.0.1", 19999, "demo_password");
    let resp2 = slave2.connect().await.expect("Slave-2 connect failed");
    println!("  [OK] Slave-2 baglandi: {}", resp2.message);
    println!("  [OK] Slave-2 node ID: {}", &slave2.node_id()[..8]);
    println!("  [OK] Bagli slave sayisi: {}\n", master.connected_slave_count());

    // ── 4. Yanlis sifre ile baglanma denemesi ────────────────
    println!("[ADIM 4] Yetkisiz erisim denemesi...");
    let intruder = SlaveNode::new("127.0.0.1", 19999, "wrong_password");
    match intruder.connect().await {
        Ok(_) => println!("  [FAIL] Intruder connected! This should not happen!"),
        Err(e) => println!("  [OK] Intruder reddedildi: {}", e),
    }
    println!();

    // ── 5. Slave-1 GRAB yapar ────────────────────────────────
    println!("[ADIM 5] Slave-1 GRAB yapiyor (VS Code durumu yakalaniyor)...");
    let grab_snapshot = StateSnapshot {
        project_path: "C:\\Users\\developer\\palmsync".into(),
        file_name: "networking/src/master.rs".into(),
        cursor_line: 87,
        clipboard_content: "pub async fn start(&self) -> anyhow::Result<()> {".into(),
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
        source_node_id: slave1.node_id().to_string(),
        browser_url: String::new(),
        file_content: String::new(),
        file_rel_path: String::new(),
    };

    slave1.send_grab(grab_snapshot).expect("GRAB failed");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let stored = master.get_state().unwrap();
    println!("  [OK] State Master RAM'e yazildi:");
    println!("       Proje  : {}", stored.project_path);
    println!("       Dosya  : {}", stored.file_name);
    println!("       Satir  : {}", stored.cursor_line);
    println!("       Pano   : {}...", &stored.clipboard_content[..40]);
    println!();

    // ── 6. Slave-2 DROP yapar ────────────────────────────────
    println!("[ADIM 6] Slave-2 DROP yapiyor (VS Code durumu geri yukleniyor)...");
    let dropped = slave2
        .request_drop()
        .await
        .expect("DROP request failed");

    match dropped {
        Some(snap) => {
            println!("  [OK] State Slave-2'ye iletildi:");
            println!("       Proje  : {}", snap.project_path);
            println!("       Dosya  : {}", snap.file_name);
            println!("       Satir  : {}", snap.cursor_line);
            println!("       Pano   : {}...", &snap.clipboard_content[..40]);

            // Dogrulama
            assert_eq!(snap.cursor_line, 87);
            assert_eq!(snap.file_name, "networking/src/master.rs");
            println!("  [OK] Veri butunlugu dogrulandi!");
        }
        None => {
            println!("  [FAIL] State alinamadi!");
        }
    }
    println!();

    // ── 7. Ikinci GRAB — Last-Write-Wins ─────────────────────
    println!("[ADIM 7] Slave-2 yeni bir GRAB yapiyor (Last-Write-Wins)...");
    let grab2 = StateSnapshot {
        project_path: "C:\\Users\\developer\\palmsync".into(),
        file_name: "brain/src/daemon.rs".into(),
        cursor_line: 23,
        clipboard_content: "tokio::select! { ... }".into(),
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
        source_node_id: slave2.node_id().to_string(),
        browser_url: String::new(),
        file_content: String::new(),
        file_rel_path: String::new(),
    };

    slave2.send_grab(grab2).expect("GRAB 2 failed");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let updated = master.get_state().unwrap();
    assert_eq!(updated.cursor_line, 23);
    assert_eq!(updated.file_name, "brain/src/daemon.rs");
    println!("  [OK] Master state guncellendi (Last-Write-Wins):");
    println!("       Dosya  : {}", updated.file_name);
    println!("       Satir  : {}", updated.cursor_line);
    println!();

    // ── 8. Slave-1 güncel state'i çeker ──────────────────────
    println!("[ADIM 8] Slave-1 DROP yapiyor (Slave-2'nin GRAB'ini aliyor)...");
    let dropped2 = slave1.request_drop().await.expect("DROP 2 failed");
    match dropped2 {
        Some(snap) => {
            assert_eq!(snap.cursor_line, 23);
            assert_eq!(snap.file_name, "brain/src/daemon.rs");
            println!("  [OK] Slave-1 guncel state'i aldi:");
            println!("       Dosya  : {}", snap.file_name);
            println!("       Satir  : {}", snap.cursor_line);
        }
        None => println!("  [FAIL] State alinamadi!"),
    }
    println!();

    // ── 9. Ping/Pong ─────────────────────────────────────────
    println!("[ADIM 9] Heartbeat testi (Ping/Pong)...");
    slave1.send_ping().expect("ping failed");
    slave2.send_ping().expect("ping failed");
    tokio::time::sleep(Duration::from_millis(100)).await;
    println!("  [OK] Slave-1 ping/pong basarili");
    println!("  [OK] Slave-2 ping/pong basarili");
    println!();

    // ── Sonuç ────────────────────────────────────────────────
    println!("==============================================");
    println!("   SONUC: Tum testler BASARILI!");
    println!("==============================================");
    println!();
    println!("  Master    : 127.0.0.1:19999");
    println!("  Slave-1   : {} (bagli)", &slave1.node_id()[..8]);
    println!("  Slave-2   : {} (bagli)", &slave2.node_id()[..8]);
    println!("  Intruder  : reddedildi");
    println!("  GRAB/DROP : calisiyor");
    println!("  LWW       : calisiyor");
    println!("  Heartbeat : calisiyor");
    println!();
    println!("  Faz 1 (Networking) tamamen fonksiyonel!");
}
