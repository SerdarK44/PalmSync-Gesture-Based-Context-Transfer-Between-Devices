use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use palmsync_shared::crypto::compute_password_hash;
use palmsync_shared::{RpcMessage, StateSnapshot};

use crate::handshake::verify_handshake;
use crate::rpc;

#[derive(Debug, Clone)]
struct SlaveEntry {
    node_id: String,
    addr: SocketAddr,
    tx: tokio::sync::mpsc::UnboundedSender<String>,
}

#[derive(Clone)]
pub struct MasterNode {
    node_id: String,
    password_hash: String,
    slaves: Arc<Mutex<HashMap<String, SlaveEntry>>>,
    state: Arc<Mutex<Option<StateSnapshot>>>,
    listen_addr: String,
}

impl MasterNode {
    pub fn new(listen_addr: &str, password: &str) -> Self {
        Self {
            node_id: uuid::Uuid::new_v4().to_string(),
            password_hash: compute_password_hash(password),
            slaves: Arc::new(Mutex::new(HashMap::new())),
            state: Arc::new(Mutex::new(None)),
            listen_addr: listen_addr.to_string(),
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

    pub async fn start(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.listen_addr).await?;
        log::info!(
            "Master {} listening on {}",
            &self.node_id[..8],
            self.listen_addr
        );

        loop {
            let (stream, addr) = listener.accept().await?;
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

        // First message must be a handshake
        let first = ws_rx
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("connection closed before handshake"))??;

        let handshake_req = match first {
            Message::Text(txt) => match rpc::parse_message(&txt)? {
                RpcMessage::Handshake(req) => req,
                _ => {
                    let err = rpc::make_error(401, "expected Handshake as first message");
                    ws_tx.send(Message::Text(err.into())).await?;
                    return Ok(());
                }
            },
            _ => {
                return Ok(());
            }
        };

        let response = verify_handshake(&handshake_req, &self.password_hash, &self.node_id);
        let resp_json =
            serde_json::to_string(&RpcMessage::HandshakeResult(response.clone()))?;
        ws_tx.send(Message::Text(resp_json.into())).await?;

        if !response.accepted {
            log::warn!("Rejected handshake from {} ({})", handshake_req.node_id, addr);
            return Ok(());
        }

        let slave_node_id = handshake_req.node_id.clone();
        log::info!("Slave {} authenticated from {}", &slave_node_id[..8], addr);

        // Channel for sending messages back to this slave
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        self.slaves.lock().insert(
            slave_node_id.clone(),
            SlaveEntry {
                node_id: slave_node_id.clone(),
                addr,
                tx,
            },
        );

        // Broadcast join notification to other slaves
        self.broadcast_except(
            &slave_node_id,
            &serde_json::to_string(&RpcMessage::NodeJoined {
                node_id: slave_node_id.clone(),
            })?,
        );

        // Forward outbound messages task
        let fwd_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_tx.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        });

        // Inbound message loop
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(txt) => {
                    let txt_str: &str = &txt;
                    if let Ok(rpc_msg) = rpc::parse_message(txt_str) {
                        self.handle_rpc(&slave_node_id, rpc_msg);
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        // Cleanup
        fwd_task.abort();
        self.slaves.lock().remove(&slave_node_id);
        log::info!("Slave {} disconnected", &slave_node_id[..8]);

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
                log::info!(
                    "GRAB from {} — {}:{} line {}",
                    &sender_id[..8],
                    snapshot.project_path,
                    snapshot.file_name,
                    snapshot.cursor_line,
                );
                *self.state.lock() = Some(snapshot);
            }
            RpcMessage::RequestDrop => {
                log::info!("DROP requested by {}", &sender_id[..8]);
                let snapshot = self.state.lock().clone();
                let resp = rpc::make_drop_response(snapshot);
                if let Some(slave) = self.slaves.lock().get(sender_id) {
                    let _ = slave.tx.send(resp);
                }
            }
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
