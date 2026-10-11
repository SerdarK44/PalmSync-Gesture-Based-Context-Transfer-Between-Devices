use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use palmsync_shared::audit;
use palmsync_shared::crypto::{compute_password_hash, CryptoManager};
use palmsync_shared::validation::Validator;
use palmsync_shared::{DeviceReport, RpcMessage, StateSnapshot};

/// Log target for device reports. The daemon's logger routes this target to
/// its own file (`cihaz-raporlari.log`) on the host, so problems from every
/// machine in the session are collected in one place.
pub const DEVICE_REPORT_TARGET: &str = "palmsync_report";

/// Write a (sanitized) device report into the host's device-report log.
pub fn log_device_report(r: &DeviceReport) {
    let level = match r.level.as_str() {
        "error" => log::Level::Error,
        "warn" => log::Level::Warn,
        _ => log::Level::Info,
    };
    log::log!(target: DEVICE_REPORT_TARGET, level, "[{}] {}: {}", r.machine, r.source, r.message);
}

use crate::handshake;
use crate::rate_limiter::{RateLimiter, MAX_PAYLOAD_SIZE};
use crate::rpc;

#[derive(Clone)]
struct SlaveEntry {
    tx: tokio::sync::mpsc::UnboundedSender<String>,
}

#[derive(Clone)]
pub struct MasterNode {
    node_id: String,
    password_hash: String,
    crypto: Arc<CryptoManager>,
    slaves: Arc<Mutex<HashMap<String, SlaveEntry>>>,
    state: Arc<Mutex<Option<StateSnapshot>>>,
    listen_addr: String,
    conn_limiter: RateLimiter,
    msg_limiter: RateLimiter,
}

impl MasterNode {
    pub fn new(listen_addr: &str, password: &str) -> Self {
        Self::new_with_limits(
            listen_addr,
            password,
            RateLimiter::for_connections(),
            RateLimiter::for_messages(),
        )
    }

    /// Custom rate limits — used by load tests and high-density deployments
    /// (many nodes behind one NAT IP).
    pub fn new_with_limits(
        listen_addr: &str,
        password: &str,
        conn_limiter: RateLimiter,
        msg_limiter: RateLimiter,
    ) -> Self {
        Self {
            node_id: uuid::Uuid::new_v4().to_string(),
            password_hash: compute_password_hash(password),
            crypto: Arc::new(CryptoManager::new(password)),
            slaves: Arc::new(Mutex::new(HashMap::new())),
            state: Arc::new(Mutex::new(None)),
            listen_addr: listen_addr.to_string(),
            conn_limiter,
            msg_limiter,
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn connected_slave_count(&self) -> usize {
        self.slaves.lock().len()
    }

    pub fn get_state(&self) -> Option<StateSnapshot> {
        self.state.lock().clone()
    }

    pub fn set_state(&self, snapshot: StateSnapshot) {
        *self.state.lock() = Some(snapshot);
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.listen_addr).await?;
        log::info!(
            "Master {} listening on {}",
            palmsync_shared::short(&self.node_id, 8),
            self.listen_addr
        );

        loop {
            let (stream, addr) = listener.accept().await?;

            // Per-IP connection rate limiting (DoS protection)
            if let Err(e) = self.conn_limiter.check(&addr.ip().to_string()) {
                audit::log_security("conn_rate_limit", &addr.to_string());
                log::warn!("Connection from {} rejected: {}", addr, e);
                continue;
            }

            log::info!("Incoming connection from {}", addr);

            let master = self.clone();
            tokio::spawn(async move {
                if let Err(e) = master.handle_connection(stream, addr).await {
                    log::error!("Connection {} error: {}", addr, e);
                }
            });
        }
    }

    async fn handle_connection(
        &self,
        stream: TcpStream,
        addr: SocketAddr,
    ) -> anyhow::Result<()> {
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        // ── Challenge-response handshake (all plaintext, no reusable secret) ──

        // Step 1: receive HandshakeInit
        let first = ws_rx
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("connection closed before handshake"))??;

        let init = match first {
            Message::Text(txt) => {
                if txt.len() > MAX_PAYLOAD_SIZE {
                    audit::log_security("oversized_handshake", &addr.to_string());
                    return Ok(());
                }
                match rpc::parse_message(&txt)? {
                    RpcMessage::HandshakeInit(init) => init,
                    _ => {
                        let err = rpc::make_error(401, "expected HandshakeInit");
                        ws_tx.send(Message::Text(err)).await?;
                        return Ok(());
                    }
                }
            }
            _ => return Ok(()),
        };

        if Validator::validate_node_id(&init.node_id).is_err() {
            audit::log_security("invalid_node_id", &addr.to_string());
            return Ok(());
        }

        // Step 2: send a fresh challenge nonce
        let nonce = handshake::generate_nonce();
        ws_tx
            .send(Message::Text(handshake::wrap_challenge(&nonce, &self.node_id)))
            .await?;

        // Step 3: receive the proof (bounded wait — no half-open sockets)
        let proof_msg = match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            ws_rx.next(),
        )
        .await
        {
            Ok(Some(Ok(Message::Text(txt)))) if txt.len() <= MAX_PAYLOAD_SIZE => {
                match rpc::parse_message(&txt)? {
                    RpcMessage::HandshakeProof(p) => p,
                    _ => {
                        audit::log_auth(&init.node_id, &addr.to_string(), false);
                        return Ok(());
                    }
                }
            }
            _ => {
                audit::log_security("handshake_timeout", &addr.to_string());
                return Ok(());
            }
        };

        // Step 4: verify and answer
        let accepted = proof_msg.node_id == init.node_id
            && handshake::verify_proof(&proof_msg.proof, &self.password_hash, &nonce);

        let msg = if accepted {
            format!("Welcome node {}", init.node_id)
        } else {
            "Authentication failed: invalid proof".to_string()
        };
        ws_tx
            .send(Message::Text(handshake::make_result(
                accepted,
                &msg,
                &self.node_id,
            )))
            .await?;

        audit::log_auth(&init.node_id, &addr.to_string(), accepted);

        if !accepted {
            log::warn!("Rejected handshake from {} ({})", init.node_id, addr);
            return Ok(());
        }

        let slave_node_id = init.node_id.clone();
        log::info!("Slave {} authenticated from {}", palmsync_shared::short(&slave_node_id, 8), addr);

        // Channel for sending messages back to this slave.
        // Payloads are plaintext JSON here; sealed just before the wire.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        self.slaves
            .lock()
            .insert(slave_node_id.clone(), SlaveEntry { tx });

        self.broadcast_except(
            &slave_node_id,
            &serde_json::to_string(&RpcMessage::NodeJoined {
                node_id: slave_node_id.clone(),
            })?,
        );

        // Outbound: seal each message with AES-256-GCM before sending
        let crypto_out = self.crypto.clone();
        let fwd_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let sealed = match crypto_out.seal(&msg) {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("Outbound seal failed: {}", e);
                        continue;
                    }
                };
                if ws_tx.send(Message::Text(sealed)).await.is_err() {
                    break;
                }
            }
        });

        // Inbound: all post-handshake messages are encrypted envelopes
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(txt) => {
                    if txt.len() > MAX_PAYLOAD_SIZE {
                        audit::log_security("oversized_payload", &slave_node_id);
                        log::warn!("Dropping oversized payload from {}", palmsync_shared::short(&slave_node_id, 8));
                        continue;
                    }
                    if self.msg_limiter.check(&slave_node_id).is_err() {
                        audit::log_security("msg_rate_limit", &slave_node_id);
                        log::warn!("Rate limit hit by {}", palmsync_shared::short(&slave_node_id, 8));
                        continue;
                    }

                    let txt_str: &str = &txt;
                    match self.crypto.open(txt_str) {
                        Ok(plain) => {
                            if let Ok(rpc_msg) = rpc::parse_message(&plain) {
                                self.handle_rpc(&slave_node_id, rpc_msg);
                            }
                        }
                        Err(e) => {
                            audit::log_security("decrypt_failed", &slave_node_id);
                            log::warn!(
                                "Undecryptable message from {}: {}",
                                palmsync_shared::short(&slave_node_id, 8),
                                e
                            );
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        // Cleanup
        fwd_task.abort();
        self.slaves.lock().remove(&slave_node_id);
        log::info!("Slave {} disconnected", palmsync_shared::short(&slave_node_id, 8));

        self.broadcast_except(
            &slave_node_id,
            &serde_json::to_string(&RpcMessage::NodeLeft {
                node_id: slave_node_id.clone(),
            })?,
        );

        Ok(())
    }

    fn handle_rpc(&self, sender_id: &str, msg: RpcMessage) {
        match msg {
            RpcMessage::Grab(snapshot) => {
                if let Err(e) = Validator::validate_snapshot(&snapshot) {
                    audit::log_security("invalid_snapshot", sender_id);
                    log::warn!("Rejected invalid snapshot from {}: {}", palmsync_shared::short(sender_id, 8), e);
                    return;
                }
                log::info!(
                    "GRAB from {} — {}:{} line {}",
                    palmsync_shared::short(sender_id, 8),
                    snapshot.project_path,
                    snapshot.file_name,
                    snapshot.cursor_line,
                );
                audit::log_sync(sender_id, "grab", &snapshot.file_name);
                *self.state.lock() = Some(snapshot);
            }
            RpcMessage::RequestDrop => {
                log::info!("DROP requested by {}", palmsync_shared::short(sender_id, 8));
                audit::log_sync(sender_id, "drop_request", "");
                let snapshot = self.state.lock().clone();
                let resp = rpc::make_drop_response(snapshot);
                if let Some(slave) = self.slaves.lock().get(sender_id) {
                    let _ = slave.tx.send(resp);
                }
            }
            RpcMessage::Report(report) => match Validator::sanitize_report(&report) {
                Ok(r) => log_device_report(&r),
                Err(e) => {
                    log::warn!("Rejected device report from {}: {}", palmsync_shared::short(sender_id, 8), e);
                }
            },
            RpcMessage::Ping => {
                if let Some(slave) = self.slaves.lock().get(sender_id) {
                    let _ = slave.tx.send(rpc::make_pong());
                }
            }
            _ => {}
        }
    }

    fn broadcast_except(&self, exclude_id: &str, msg: &str) {
        let slaves = self.slaves.lock();
        for (id, entry) in slaves.iter() {
            if id != exclude_id {
                let _ = entry.tx.send(msg.to_string());
            }
        }
    }
}
