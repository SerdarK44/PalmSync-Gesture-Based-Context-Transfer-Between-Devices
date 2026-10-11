use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use palmsync_shared::crypto::CryptoManager;
use palmsync_shared::validation::Validator;
use palmsync_shared::{DeviceReport, HandshakeResponse, RpcMessage, StateSnapshot};

use crate::handshake::{build_init, compute_proof, wrap_init, wrap_proof};
use crate::rpc;

#[derive(Clone)]
pub struct SlaveNode {
    node_id: String,
    master_url: String,
    password: String,
    crypto: Arc<CryptoManager>,
    outbound_tx: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>>,
    latest_drop: Arc<Mutex<Option<StateSnapshot>>>,
    connected: Arc<Mutex<bool>>,
    drop_signal: Arc<tokio::sync::Notify>,
    shutdown: Arc<tokio::sync::Notify>,
}

impl SlaveNode {
    pub fn new(master_addr: &str, master_port: u16, password: &str) -> Self {
        Self {
            node_id: uuid::Uuid::new_v4().to_string(),
            master_url: format!("ws://{}:{}", master_addr, master_port),
            password: password.to_string(),
            crypto: Arc::new(CryptoManager::new(password)),
            outbound_tx: Arc::new(Mutex::new(None)),
            latest_drop: Arc::new(Mutex::new(None)),
            connected: Arc::new(Mutex::new(false)),
            drop_signal: Arc::new(tokio::sync::Notify::new()),
            shutdown: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Close the connection: stops both background tasks so the
    /// underlying TCP stream is dropped and the master cleans up.
    pub fn disconnect(&self) {
        *self.outbound_tx.lock() = None; // ends the send task
        // notify_one stores a permit, so the inbound task shuts down even
        // if it has not reached its select await yet
        self.shutdown.notify_one();
        *self.connected.lock() = false;
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn is_connected(&self) -> bool {
        *self.connected.lock()
    }

    pub async fn connect(&self) -> anyhow::Result<HandshakeResponse> {
        log::info!("Connecting to master at {}", self.master_url);

        let (ws_stream, _) = tokio_tungstenite::connect_async(&self.master_url).await?;
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        // ── Challenge-response handshake ──

        // Step 1: announce identity (no secret on the wire)
        ws_tx
            .send(Message::Text(wrap_init(build_init(&self.node_id))))
            .await?;

        // Step 2: receive the challenge nonce
        let challenge_msg = ws_rx
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("connection closed during handshake"))??;
        let challenge = match challenge_msg {
            Message::Text(txt) => match rpc::parse_message(&txt)? {
                RpcMessage::HandshakeChallenge(c) => c,
                RpcMessage::Error { message, .. } => {
                    return Err(anyhow::anyhow!("handshake rejected: {}", message))
                }
                _ => return Err(anyhow::anyhow!("expected HandshakeChallenge")),
            },
            _ => return Err(anyhow::anyhow!("unexpected message type during handshake")),
        };

        // Step 3: answer with proof = SHA-256(password_hash : nonce)
        let proof = compute_proof(&self.password, &challenge.nonce);
        ws_tx
            .send(Message::Text(wrap_proof(&self.node_id, &proof)))
            .await?;

        // Step 4: receive accept / reject
        let resp_msg = ws_rx
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("connection closed during handshake"))??;
        let hs_resp = match resp_msg {
            Message::Text(txt) => match rpc::parse_message(&txt)? {
                RpcMessage::HandshakeResult(resp) => resp,
                _ => return Err(anyhow::anyhow!("unexpected response during handshake")),
            },
            _ => return Err(anyhow::anyhow!("unexpected message type during handshake")),
        };

        if !hs_resp.accepted {
            return Err(anyhow::anyhow!(
                "Handshake rejected: {}",
                hs_resp.message
            ));
        }

        log::info!("Authenticated with master {}", palmsync_shared::short(&hs_resp.master_node_id, 8));
        *self.connected.lock() = true;

        // Outbound channel: plaintext JSON in, sealed on the wire
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        *self.outbound_tx.lock() = Some(tx);

        let crypto_out = self.crypto.clone();
        let send_task = tokio::spawn(async move {
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
        let connected = self.connected.clone();
        let latest_drop = self.latest_drop.clone();
        let drop_signal = self.drop_signal.clone();
        let crypto_in = self.crypto.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            loop {
                let msg = tokio::select! {
                    m = ws_rx.next() => match m {
                        Some(Ok(m)) => m,
                        _ => break,
                    },
                    _ = shutdown.notified() => break,
                };
                match msg {
                    Message::Text(txt) => {
                        let txt_str: &str = &txt;
                        let plain = match crypto_in.open(txt_str) {
                            Ok(p) => p,
                            Err(e) => {
                                log::warn!("Undecryptable message from master: {}", e);
                                continue;
                            }
                        };
                        if let Ok(rpc_msg) = rpc::parse_message(&plain) {
                            match rpc_msg {
                                RpcMessage::DropResponse(snap) => {
                                    // Validate before trusting network data
                                    let snap = snap.filter(|s| {
                                        match Validator::validate_snapshot(s) {
                                            Ok(()) => true,
                                            Err(e) => {
                                                log::warn!("Rejected invalid drop snapshot: {}", e);
                                                false
                                            }
                                        }
                                    });
                                    *latest_drop.lock() = snap;
                                    drop_signal.notify_one();
                                }
                                RpcMessage::Pong => {
                                    log::debug!("Pong received");
                                }
                                RpcMessage::NodeJoined { node_id } => {
                                    log::info!("Node {} joined the network", palmsync_shared::short(&node_id, 8));
                                }
                                RpcMessage::NodeLeft { node_id } => {
                                    log::info!("Node {} left the network", palmsync_shared::short(&node_id, 8));
                                }
                                _ => {}
                            }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }

            *connected.lock() = false;
            send_task.abort();
            log::warn!("Disconnected from master");
        });

        Ok(hs_resp)
    }

    pub fn send_grab(&self, snapshot: StateSnapshot) -> anyhow::Result<()> {
        Validator::validate_snapshot(&snapshot)?;
        let tx = self.outbound_tx.lock();
        let tx = tx
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("not connected"))?;
        tx.send(rpc::make_grab(snapshot))?;
        Ok(())
    }

    /// Send a problem report to the host (fire-and-forget, like GRAB).
    pub fn send_report(&self, report: DeviceReport) -> anyhow::Result<()> {
        let report = Validator::sanitize_report(&report)?;
        let tx = self.outbound_tx.lock();
        let tx = tx
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("not connected"))?;
        tx.send(serde_json::to_string(&RpcMessage::Report(report))?)?;
        Ok(())
    }

    pub async fn request_drop(&self) -> anyhow::Result<Option<StateSnapshot>> {
        {
            let tx = self.outbound_tx.lock();
            let tx = tx
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("not connected"))?;
            tx.send(rpc::make_drop_request())?;
        }

        // Wait for the drop response (with timeout)
        tokio::select! {
            _ = self.drop_signal.notified() => {
                Ok(self.latest_drop.lock().take())
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                Err(anyhow::anyhow!("drop request timed out"))
            }
        }
    }

    pub fn send_ping(&self) -> anyhow::Result<()> {
        let tx = self.outbound_tx.lock();
        let tx = tx
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("not connected"))?;
        tx.send(rpc::make_ping())?;
        Ok(())
    }
}
