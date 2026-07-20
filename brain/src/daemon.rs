use palmsync_shared::{Config, GestureEvent};

use palmsync_networking::{MasterNode, SlaveNode};

use crate::context_capture;
use crate::executor;
use crate::ipc_listener::IpcListener;

pub async fn run_master(config: Config) -> anyhow::Result<()> {
    let master = MasterNode::new(&config.listen_addr(), &config.password);

    let ipc = IpcListener::bind(config.ipc_port).await?;
    let master_for_ipc = master.clone();
    let node_id = config.node_id.clone();

    // IPC listener task — handles local gestures on the master machine too
    tokio::spawn(async move {
        loop {
            match ipc.recv_gesture().await {
                Ok(packet) => match packet.event {
                    GestureEvent::Grab => {
                        log::info!("Local GRAB on master");
                        match context_capture::capture_vscode_context(&node_id) {
                            Ok(snap) => {
                                let mut state = master_for_ipc.get_state();
                                let _ = state.insert(snap.clone());
                                log::info!(
                                    "State captured: {}:{} line {}",
                                    snap.project_path,
                                    snap.file_name,
                                    snap.cursor_line
                                );
                            }
                            Err(e) => log::warn!("Context capture failed: {}", e),
                        }
                    }
                    GestureEvent::Drop => {
                        log::info!("Local DROP on master");
                        if let Some(snap) = master_for_ipc.get_state() {
                            if let Err(e) = crate::clipboard::set_text(&snap.clipboard_content) {
                                log::warn!("Clipboard set failed: {}", e);
                            }
                            if let Err(e) = executor::open_vscode(
                                &snap.project_path,
                                &snap.file_name,
                                snap.cursor_line,
                            )
                            .await
                            {
                                log::warn!("VS Code open failed: {}", e);
                            }
                        } else {
                            log::info!("No state available for DROP");
                        }
                    }
                },
                Err(e) => {
                    log::error!("IPC recv error: {}", e);
                }
            }
        }
    });

    // Run the WebSocket server (blocks)
    master.start().await
}

pub async fn run_slave(config: Config) -> anyhow::Result<()> {
    let slave = SlaveNode::new(&config.master_addr, config.master_port, &config.password);

    // Connect with retry
    loop {
        match slave.connect().await {
            Ok(resp) => {
                log::info!("Connected to master: {}", resp.message);
                break;
            }
            Err(e) => {
                log::warn!("Connection failed: {} — retrying in 3s", e);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }
    }

    let ipc = IpcListener::bind(config.ipc_port).await?;

    loop {
        match ipc.recv_gesture().await {
            Ok(packet) => match packet.event {
                GestureEvent::Grab => {
                    log::info!("GRAB event received");
                    match context_capture::capture_vscode_context(&config.node_id) {
                        Ok(snap) => {
                            if let Err(e) = slave.send_grab(snap) {
                                log::error!("Failed to send grab: {}", e);
                            }
                        }
                        Err(e) => log::warn!("Context capture failed: {}", e),
                    }
                }
                GestureEvent::Drop => {
                    log::info!("DROP event received");
                    match slave.request_drop().await {
                        Ok(Some(snap)) => {
                            log::info!(
                                "Restoring: {}:{} line {}",
                                snap.project_path,
                                snap.file_name,
                                snap.cursor_line
                            );
                            if let Err(e) = crate::clipboard::set_text(&snap.clipboard_content) {
                                log::warn!("Clipboard set failed: {}", e);
                            }
                            if let Err(e) = executor::open_vscode(
                                &snap.project_path,
                                &snap.file_name,
                                snap.cursor_line,
                            )
                            .await
                            {
                                log::warn!("VS Code open failed: {}", e);
                            }
                        }
                        Ok(None) => {
                            log::info!("No state available on master");
                        }
                        Err(e) => {
                            log::error!("Drop request failed: {}", e);
                        }
                    }
                }
            },
            Err(e) => {
                log::error!("IPC recv error: {}", e);
            }
        }
    }
}
