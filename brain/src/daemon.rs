use palmsync_shared::{Config, GestureEvent};

use palmsync_networking::{MasterNode, SlaveNode};

use crate::context_capture;
use crate::executor;
use crate::ipc_listener::IpcListener;
use crate::notifications;

pub async fn run_master(config: Config) -> anyhow::Result<()> {
    let master = MasterNode::new(&config.bind_addr(), &config.password);

    let ipc = IpcListener::bind(config.ipc_port).await?;
    let master_for_ipc = master.clone();
    let node_id = config.node_id.clone();

    tokio::spawn(async move {
        loop {
            match ipc.recv_gesture().await {
                Ok(packet) => {
                    log::info!(
                        "Gesture {:?} (confidence: {:.2})",
                        packet.event,
                        packet.confidence
                    );
                    match packet.event {
                        GestureEvent::Grab => {
                            handle_grab_master(&master_for_ipc, &node_id).await;
                        }
                        GestureEvent::Drop => {
                            handle_drop_local(master_for_ipc.get_state()).await;
                        }
                    }
                }
                Err(e) => {
                    log::error!("IPC recv error: {}", e);
                }
            }
        }
    });

    master.start().await
}

async fn handle_grab_master(master: &MasterNode, node_id: &str) {
    match context_capture::capture_vscode_context(node_id) {
        Ok(snap) => {
            log::info!(
                "GRAB captured: {}:{} line {}",
                snap.project_path,
                snap.file_name,
                snap.cursor_line
            );
            notifications::notify_grab(&snap.file_name, snap.cursor_line);
            master.set_state(snap);
        }
        Err(e) => log::warn!("Context capture failed: {}", e),
    }
}

async fn handle_drop_local(state: Option<palmsync_shared::StateSnapshot>) {
    match state {
        Some(snap) => {
            log::info!(
                "DROP restoring: {}:{} line {}",
                snap.project_path,
                snap.file_name,
                snap.cursor_line
            );
            notifications::notify_drop(&snap.file_name, snap.cursor_line);
            if let Err(e) = crate::clipboard::set_text(&snap.clipboard_content) {
                log::warn!("Clipboard set failed: {}", e);
            }
            if let Err(e) =
                executor::open_vscode(&snap.project_path, &snap.file_name, snap.cursor_line).await
            {
                log::warn!("VS Code open failed: {}", e);
            }
        }
        None => {
            log::info!("No state available for DROP");
        }
    }
}

pub async fn run_slave(config: Config) -> anyhow::Result<()> {
    let slave = SlaveNode::new(&config.master_addr, config.master_port, &config.password);

    loop {
        match slave.connect().await {
            Ok(resp) => {
                log::info!("Connected to master: {}", resp.message);
                notifications::notify_connected(&config.master_addr);
                break;
            }
            Err(e) => {
                log::warn!("Connection failed: {} — retrying in 3s", e);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }
    }

    let ipc = IpcListener::bind(config.ipc_port).await?;
    log::info!("Slave ready — waiting for gestures on UDP :{}", config.ipc_port);

    loop {
        match ipc.recv_gesture().await {
            Ok(packet) => {
                log::info!(
                    "Gesture {:?} (confidence: {:.2})",
                    packet.event,
                    packet.confidence
                );
                match packet.event {
                    GestureEvent::Grab => {
                        handle_grab_slave(&slave, &config.node_id).await;
                    }
                    GestureEvent::Drop => {
                        handle_drop_slave(&slave).await;
                    }
                }
            }
            Err(e) => {
                log::error!("IPC recv error: {}", e);
            }
        }
    }
}

async fn handle_grab_slave(slave: &SlaveNode, node_id: &str) {
    match context_capture::capture_vscode_context(node_id) {
        Ok(snap) => {
            log::info!(
                "GRAB captured: {}:{} line {}",
                snap.project_path,
                snap.file_name,
                snap.cursor_line
            );
            notifications::notify_grab(&snap.file_name, snap.cursor_line);
            if let Err(e) = slave.send_grab(snap) {
                log::error!("Failed to send grab to master: {}", e);
            }
        }
        Err(e) => log::warn!("Context capture failed: {}", e),
    }
}

async fn handle_drop_slave(slave: &SlaveNode) {
    match slave.request_drop().await {
        Ok(Some(snap)) => {
            log::info!(
                "DROP restoring: {}:{} line {}",
                snap.project_path,
                snap.file_name,
                snap.cursor_line
            );
            notifications::notify_drop(&snap.file_name, snap.cursor_line);
            if let Err(e) = crate::clipboard::set_text(&snap.clipboard_content) {
                log::warn!("Clipboard set failed: {}", e);
            }
            if let Err(e) =
                executor::open_vscode(&snap.project_path, &snap.file_name, snap.cursor_line).await
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
