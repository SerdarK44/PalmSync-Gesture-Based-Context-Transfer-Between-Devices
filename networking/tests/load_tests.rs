//! Load tests — run manually with:
//!   cargo test -p palmsync-networking --test load_tests -- --ignored --nocapture

use std::time::{Duration, Instant};

use palmsync_networking::{MasterNode, RateLimiter, SlaveNode};
use palmsync_shared::{RpcMessage, StateSnapshot};

fn snapshot(node: usize, seq: u32) -> StateSnapshot {
    StateSnapshot {
        project_path: format!("C:\\projects\\load-{}", node),
        file_name: format!("file{}.rs", seq),
        cursor_line: seq,
        clipboard_content: format!("payload-{}-{}", node, seq),
        timestamp: 1_700_000_000_000 + seq as u64,
        source_node_id: format!("load-node-{}", node),
    }
}

/// 100+ concurrent slaves from one IP: connect, each sends 10 grabs.
/// Success criterion from the phase doc: >= 95% connection success.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore]
async fn load_100_concurrent_connections() {
    // Generous limits: local load test shares a single loopback IP
    let master = MasterNode::new_with_limits(
        "127.0.0.1:29201",
        "load_pass",
        RateLimiter::new(10_000, 60),
        RateLimiter::new(100_000, 10),
    );
    let m = master.clone();
    tokio::spawn(async move {
        let _ = m.start().await;
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    const SLAVES: usize = 100;
    const GRABS_PER_SLAVE: u32 = 10;

    let start = Instant::now();
    let mut handles = Vec::with_capacity(SLAVES);

    for i in 0..SLAVES {
        handles.push(tokio::spawn(async move {
            let slave = SlaveNode::new("127.0.0.1", 29201, "load_pass");
            slave.connect().await.map_err(|e| format!("connect: {}", e))?;

            for j in 0..GRABS_PER_SLAVE {
                slave
                    .send_grab(snapshot(i, j))
                    .map_err(|e| format!("grab: {}", e))?;
            }
            Ok::<(), String>(())
        }));
    }

    let mut ok = 0;
    let mut failed = 0;
    for h in handles {
        match h.await {
            Ok(Ok(())) => ok += 1,
            _ => failed += 1,
        }
    }

    let elapsed = start.elapsed();
    println!(
        "Load: {}/{} slaves OK ({} failed) in {:.2}s — {} grabs total",
        ok,
        SLAVES,
        failed,
        elapsed.as_secs_f64(),
        ok * GRABS_PER_SLAVE as usize,
    );

    // Give the master a moment to drain inbound queues
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(master.get_state().is_some(), "master must hold final state");
    assert!(
        ok as f64 / SLAVES as f64 >= 0.95,
        "success rate below 95%: {}/{}",
        ok,
        SLAVES
    );
}

/// Serialization throughput baseline: JSON roundtrip must exceed 10k msg/s.
#[tokio::test]
#[ignore]
async fn load_message_throughput() {
    const N: usize = 10_000;
    let start = Instant::now();

    for i in 0..N {
        let msg = RpcMessage::Grab(snapshot(1, i as u32));
        let json = serde_json::to_string(&msg).unwrap();
        let _back: RpcMessage = serde_json::from_str(&json).unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = N as f64 / elapsed.as_secs_f64();
    println!("Serialization throughput: {:.0} msg/sec", throughput);
    assert!(throughput > 10_000.0, "below 10k msg/s: {:.0}", throughput);
}

/// Encrypted end-to-end throughput: one slave hammers grabs through the
/// full E2EE pipeline; master must keep up.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn load_sustained_encrypted_grabs() {
    let master = MasterNode::new_with_limits(
        "127.0.0.1:29202",
        "load_pass",
        RateLimiter::new(10_000, 60),
        RateLimiter::new(1_000_000, 10),
    );
    let m = master.clone();
    tokio::spawn(async move {
        let _ = m.start().await;
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    let slave = SlaveNode::new("127.0.0.1", 29202, "load_pass");
    slave.connect().await.unwrap();

    const N: u32 = 2_000;
    let start = Instant::now();
    for j in 0..N {
        slave.send_grab(snapshot(0, j)).unwrap();
    }

    // Wait until the last grab lands (or timeout)
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(s) = master.get_state() {
            if s.cursor_line == N - 1 {
                break;
            }
        }
        if Instant::now() > deadline {
            panic!("master did not process all grabs in time");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let elapsed = start.elapsed();
    println!(
        "E2EE grab throughput: {:.0} grabs/sec ({} grabs in {:.2}s)",
        N as f64 / elapsed.as_secs_f64(),
        N,
        elapsed.as_secs_f64()
    );
}
