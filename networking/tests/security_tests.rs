//! Wire-level security tests: attacks a real Master over WebSocket
//! and verifies the hardening added in Faz 6 holds.

use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use palmsync_networking::handshake::{build_init, compute_proof, wrap_init, wrap_proof};
use palmsync_networking::{MasterNode, RateLimiter, SlaveNode};
use palmsync_shared::crypto::CryptoManager;
use palmsync_shared::{RpcMessage, StateSnapshot};

type WsTx = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type WsRx = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

fn snapshot(line: u32) -> StateSnapshot {
    StateSnapshot {
        project_path: "C:\\Users\\dev\\project".into(),
        file_name: "main.rs".into(),
        cursor_line: line,
        clipboard_content: "attack test".into(),
        timestamp: 1,
        source_node_id: "sec-test-node".into(),
        browser_url: String::new(),
        file_content: String::new(),
        file_rel_path: String::new(),
    }
}

async fn start_master(port: u16, password: &str) -> MasterNode {
    let master = MasterNode::new(&format!("127.0.0.1:{}", port), password);
    let m = master.clone();
    tokio::spawn(async move {
        let _ = m.start().await;
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    master
}

/// Complete the challenge-response handshake on a raw split socket,
/// returning whether the master accepted. Used to set up post-auth attacks.
async fn authenticate(
    tx: &mut WsTx,
    rx: &mut WsRx,
    node_id: &str,
    password: &str,
) -> bool {
    tx.send(Message::Text(wrap_init(build_init(node_id))))
        .await
        .unwrap();

    let challenge = match rx.next().await {
        Some(Ok(Message::Text(txt))) => match serde_json::from_str::<RpcMessage>(&txt).unwrap() {
            RpcMessage::HandshakeChallenge(c) => c,
            _ => return false,
        },
        _ => return false,
    };

    let proof = compute_proof(password, &challenge.nonce);
    tx.send(Message::Text(wrap_proof(node_id, &proof)))
        .await
        .unwrap();

    match rx.next().await {
        Some(Ok(Message::Text(txt))) => match serde_json::from_str::<RpcMessage>(&txt).unwrap() {
            RpcMessage::HandshakeResult(r) => r.accepted,
            _ => false,
        },
        _ => false,
    }
}

/// An authenticated attacker sending PLAINTEXT (unencrypted) RPC messages
/// must not be able to modify master state — post-handshake traffic
/// must be sealed with the E2EE key.
#[tokio::test]
async fn plaintext_injection_after_handshake_ignored() {
    let master = start_master(29101, "sec_pass").await;

    let (ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:29101")
        .await
        .unwrap();
    let (mut tx, mut rx) = ws.split();

    // Valid handshake (attacker knows the password)
    assert!(authenticate(&mut tx, &mut rx, "evil-node-1", "sec_pass").await);

    // Inject an UNENCRYPTED Grab — must be dropped by the decrypt gate
    let plaintext_grab = serde_json::to_string(&RpcMessage::Grab(snapshot(666))).unwrap();
    tx.send(Message::Text(plaintext_grab)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        master.get_state().is_none(),
        "plaintext injection must not update master state"
    );
}

/// Messages sealed with the WRONG key (wrong password guess for E2EE
/// while replaying a stolen handshake hash) must be rejected.
#[tokio::test]
async fn wrong_key_ciphertext_rejected() {
    let master = start_master(29102, "correct_horse").await;

    let (ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:29102")
        .await
        .unwrap();
    let (mut tx, mut rx) = ws.split();

    assert!(authenticate(&mut tx, &mut rx, "evil-node-2", "correct_horse").await);

    // Seal with a DIFFERENT password-derived key
    let wrong_crypto = CryptoManager::new("battery_staple");
    let inner = serde_json::to_string(&RpcMessage::Grab(snapshot(777))).unwrap();
    let sealed_wrong = wrong_crypto.seal(&inner).unwrap();
    tx.send(Message::Text(sealed_wrong)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(master.get_state().is_none());
}

/// A tampered (bit-flipped) but otherwise valid envelope must fail
/// the AES-GCM tag check and be discarded.
#[tokio::test]
async fn tampered_envelope_rejected() {
    let master = start_master(29103, "sec_pass").await;

    let (ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:29103")
        .await
        .unwrap();
    let (mut tx, mut rx) = ws.split();

    assert!(authenticate(&mut tx, &mut rx, "evil-node-3", "sec_pass").await);

    // Build a legitimate envelope, then corrupt one hex nibble
    let crypto = CryptoManager::new("sec_pass");
    let inner = serde_json::to_string(&RpcMessage::Grab(snapshot(888))).unwrap();
    let sealed = crypto.seal(&inner).unwrap();
    let tampered = if sealed.contains("\"ciphertext\":\"a") {
        sealed.replacen("\"ciphertext\":\"a", "\"ciphertext\":\"b", 1)
    } else {
        sealed.replacen("\"ciphertext\":\"", "\"ciphertext\":\"a", 1)
    };
    tx.send(Message::Text(tampered)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(master.get_state().is_none());
}

/// Snapshots with path traversal must be rejected server-side even
/// when correctly encrypted by an authenticated node.
#[tokio::test]
async fn malicious_snapshot_rejected_by_master() {
    let master = start_master(29104, "sec_pass").await;

    let (ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:29104")
        .await
        .unwrap();
    let (mut tx, mut rx) = ws.split();

    assert!(authenticate(&mut tx, &mut rx, "evil-node-4", "sec_pass").await);

    let crypto = CryptoManager::new("sec_pass");
    let mut evil = snapshot(1);
    evil.project_path = "C:\\ok\\..\\..\\Windows\\System32".into();
    let inner = serde_json::to_string(&RpcMessage::Grab(evil)).unwrap();
    tx.send(Message::Text(crypto.seal(&inner).unwrap()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        master.get_state().is_none(),
        "traversal snapshot must be rejected"
    );
}

/// Node IDs with shell metacharacters are rejected at handshake.
#[tokio::test]
async fn invalid_node_id_connection_dropped() {
    let _master = start_master(29105, "sec_pass").await;

    let (ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:29105")
        .await
        .unwrap();
    let (mut tx, mut rx) = ws.split();

    // Invalid node_id is rejected before the challenge is even issued
    tx.send(Message::Text(wrap_init(build_init("evil;rm -rf /"))))
        .await
        .unwrap();

    // Master drops the connection without issuing a challenge:
    // acceptable outcomes are silence, a Close frame, an abrupt reset
    // (protocol error on the client side) — anything except a Text reply.
    let got = tokio::time::timeout(Duration::from_secs(2), rx.next()).await;
    // dropped / closed / reset / timeout are all fine — only a Text reply fails
    if let Ok(Some(Ok(Message::Text(txt)))) = got {
        panic!("expected drop, got response: {}", txt);
    }
}

/// Connection flooding from one IP is throttled by the connection limiter.
#[tokio::test]
async fn connection_flood_throttled() {
    // Tight limit: 3 connections per minute
    let master = MasterNode::new_with_limits(
        "127.0.0.1:29106",
        "sec_pass",
        RateLimiter::new(3, 60),
        RateLimiter::for_messages(),
    );
    let m = master.clone();
    tokio::spawn(async move {
        let _ = m.start().await;
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    let mut successes = 0;
    let mut failures = 0;
    for _ in 0..6 {
        // A full slave connect exercises the WS handshake path
        let slave = SlaveNode::new("127.0.0.1", 29106, "sec_pass");
        match tokio::time::timeout(Duration::from_secs(2), slave.connect()).await {
            Ok(Ok(_)) => successes += 1,
            _ => failures += 1,
        }
    }

    assert!(successes <= 3, "limiter must cap connections (got {})", successes);
    assert!(failures >= 3, "flood connections must fail (got {})", failures);
}

/// Sanity: the legitimate encrypted path still works end to end.
#[tokio::test]
async fn legitimate_encrypted_flow_still_works() {
    let master = start_master(29107, "sec_pass").await;

    let slave = SlaveNode::new("127.0.0.1", 29107, "sec_pass");
    slave.connect().await.unwrap();
    slave.send_grab(snapshot(42)).unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;
    let state = master.get_state().expect("legit grab must be stored");
    assert_eq!(state.cursor_line, 42);
}

/// The core win of challenge-response: a proof captured from one session
/// cannot authenticate a new connection, because the nonce is fresh.
#[tokio::test]
async fn replayed_proof_from_old_session_rejected() {
    let _master = start_master(29108, "sec_pass").await;

    // Session A: capture a legitimate proof + the nonce it answered
    let (ws_a, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:29108")
        .await
        .unwrap();
    let (mut tx_a, mut rx_a) = ws_a.split();
    tx_a.send(Message::Text(wrap_init(build_init("replayer"))))
        .await
        .unwrap();
    let nonce_a = match rx_a.next().await {
        Some(Ok(Message::Text(txt))) => match serde_json::from_str::<RpcMessage>(&txt).unwrap() {
            RpcMessage::HandshakeChallenge(c) => c.nonce,
            _ => panic!("expected challenge"),
        },
        _ => panic!("no challenge"),
    };
    let captured_proof = compute_proof("sec_pass", &nonce_a);
    // (session A is abandoned without completing)

    // Session B: replay the captured proof against a fresh challenge
    let (ws_b, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:29108")
        .await
        .unwrap();
    let (mut tx_b, mut rx_b) = ws_b.split();
    tx_b.send(Message::Text(wrap_init(build_init("replayer"))))
        .await
        .unwrap();
    let _nonce_b = rx_b.next().await.unwrap().unwrap(); // different nonce

    // Send the OLD proof — bound to nonce_a, must fail under nonce_b
    tx_b.send(Message::Text(wrap_proof("replayer", &captured_proof)))
        .await
        .unwrap();

    let result = rx_b.next().await;
    let accepted = matches!(
        result,
        Some(Ok(Message::Text(ref txt)))
            if matches!(
                serde_json::from_str::<RpcMessage>(txt),
                Ok(RpcMessage::HandshakeResult(r)) if r.accepted
            )
    );
    assert!(!accepted, "replayed proof must be rejected");
}
