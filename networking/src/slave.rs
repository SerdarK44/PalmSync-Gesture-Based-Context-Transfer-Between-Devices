use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use palmsync_shared::{HandshakeResponse, RpcMessage, StateSnapshot};

use crate::handshake::{build_handshake_request, wrap_handshake};
use crate::rpc;

#[derive(Clone)]
pub struct SlaveNode {
    node_id: String,
    master_url: String,
    password: String,
    outbound_tx: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>>,
    latest_drop: Arc<Mutex<Option<StateSnapshot>>>,
    connected: Arc<Mutex<bool>>,
    drop_signal: Arc<tokio::sync::Notify>,
}

impl SlaveNode {
    pub fn new(master_addr: &str, master_port: u16, password: &str) -> Self {
        Self {
            node_id: uuid::Uuid::new_v4().to_string(),
            master_url: format!("ws://{}:{}", master_addr, master_port),
            password: password.to_string(),
            outbound_tx: Arc::new(Mutex::new(None)),
            latest_drop: Arc::new(Mutex::new(None)),
            connected: Arc::new(Mutex::new(false)),
            drop_signal: Arc::new(tokio::sync::Notify::new()),
        }
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

        // Send handshake
        let hs_req = build_handshake_request(&self.node_id, &self.password);
        ws_tx
            .send(Message::Text(wrap_handshake(hs_req).into()))
            .await?;

        // Wait for response
        let resp_msg = ws_rx
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("connection closed during handshake"))??;

        let hs_resp = match resp_msg {
            Message::Text(txt) => {
                let txt_str: &str = &txt;
                match rpc::parse_message(txt_str)? {
                    RpcMessage::HandshakeResult(resp) => resp,
                    _ => return Err(anyhow::anyhow!("unexpected response during handshake")),
                }
            }
            _ => return Err(anyhow::anyhow!("unexpected message type during handshake")),
        };

        if !hs_resp.accepted {
            return Err(anyhow::anyhow!(
                "Handshake rejected: {}",
                hs_resp.message
            ));
        }

        log::info!("Authenticated with master {}", &hs_resp.master_node_id[..8]);
        *self.connected.lock() = true;

        // Outbound channel
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        *self.outbound_tx.lock() = Some(tx);

        // Forward outbound messages
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_tx.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        });

        // Inbound message loop
        let connected = self.connected.clone();
        let latest_drop = self.latest_drop.clone();
        let drop_signal = self.drop_signal.clone();

        tokio::spawn(async move {
            while let Some(Ok(msg)) = ws_rx.next().await {
                match msg {
                    Message::Text(txt) => {
                        let txt_str: &str = &txt;
                        if let Ok(rpc_msg) = rpc::parse_message(txt_str) {
                            match rpc_msg {
                                RpcMessage::DropResponse(snap) => {
                                    *latest_drop.lock() = snap;
                                    drop_signal.notify_one();
                                }
                                RpcMessage::Pong => {
                                    log::debug!("Pong received");
                                }
                                RpcMessage::NodeJoined { node_id } => {
                                    log::info!("Node {} joined the network", &node_id[..8]);
                                }
                                RpcMessage::NodeLeft { node_id } => {
                                    log::info!("Node {} left the network", &node_id[..8]);
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
        let tx = self.outbound_tx.lock();
        let tx = tx
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("not connected"))?;
        tx.send(rpc::make_grab(snapshot))?;
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
