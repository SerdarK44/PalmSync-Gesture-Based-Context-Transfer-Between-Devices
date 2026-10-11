use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;

use palmsync_shared::validation::Validator;
use palmsync_shared::{Config, DeviceReport, GestureEvent};

use palmsync_networking::{log_device_report, MasterNode, SlaveNode};

use crate::context_capture;
use crate::executor;
use crate::ipc_listener::{IpcListener, IpcMessage};
use crate::logging::{self, LocalProblem};
use crate::notifications;

/// Max reports a joined machine sends to the host per minute (a broken
/// camera retrying in a loop must not flood the host's log).
const REPORTS_PER_MINUTE: u32 = 30;
/// The same message is forwarded at most once per this window.
const REPORT_DEDUPE_MS: u64 = 60_000;
/// Reports kept while not (yet) connected to the host.
const REPORT_QUEUE_MAX: usize = 200;

fn machine_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "?".into())
}

fn problem_to_report(p: LocalProblem) -> DeviceReport {
    DeviceReport {
        machine: machine_name(),
        level: if p.level == log::Level::Error { "error" } else { "warn" }.into(),
        source: "daemon".into(),
        message: p.message,
        timestamp: now_ms(),
    }
}

/// On the host, its own problems go straight into the device-report log.
fn spawn_local_report_logger() {
    let mut problems = logging::capture_problems();
    tokio::spawn(async move {
        while let Some(p) = problems.recv().await {
            if let Ok(r) = Validator::sanitize_report(&problem_to_report(p)) {
                log_device_report(&r);
            }
        }
    });
}

/// On a joined machine: queue reports (own daemon problems + the UI's system
/// check) and deliver them to the host whenever the link is up — so a
/// problem that happened before/while disconnected still reaches the host.
fn spawn_report_forwarder(slave: SlaveNode) -> mpsc::UnboundedSender<DeviceReport> {
    let (tx, mut rx) = mpsc::unbounded_channel::<DeviceReport>();

    let mut problems = logging::capture_problems();
    let own = tx.clone();
    tokio::spawn(async move {
        while let Some(p) = problems.recv().await {
            let _ = own.send(problem_to_report(p));
        }
    });

    tokio::spawn(async move {
        let mut queue: VecDeque<DeviceReport> = VecDeque::new();
        let mut recent: HashMap<String, u64> = HashMap::new();
        let mut window_start = now_ms();
        let mut sent_in_window = 0u32;
        let mut tick = tokio::time::interval(Duration::from_secs(3));

        loop {
            tokio::select! {
                Some(r) = rx.recv() => {
                    let Ok(r) = Validator::sanitize_report(&r) else { continue };
                    let now = now_ms();
                    let key = format!("{}|{}", r.source, r.message);
                    if recent.get(&key).is_some_and(|t| now.saturating_sub(*t) < REPORT_DEDUPE_MS) {
                        continue;
                    }
                    recent.insert(key, now);
                    if recent.len() > 500 {
                        recent.retain(|_, t| now.saturating_sub(*t) < REPORT_DEDUPE_MS);
                    }
                    if queue.len() >= REPORT_QUEUE_MAX {
                        queue.pop_front();
                    }
                    queue.push_back(r);
                }
                _ = tick.tick() => {}
            }

            let now = now_ms();
            if now.saturating_sub(window_start) >= 60_000 {
                window_start = now;
                sent_in_window = 0;
            }
            while sent_in_window < REPORTS_PER_MINUTE {
                let Some(r) = queue.front() else { break };
                if slave.send_report(r.clone()).is_err() {
                    break; // not connected: keep it queued
                }
                queue.pop_front();
                sent_in_window += 1;
            }
        }
    });
    tx
}

/// Minimum time between two accepted GRAB captures. The camera fires a gesture
/// on every fist it sees; without this the daemon would capture many times a
/// second while the user holds a fist. One capture per 5 s is plenty.
const GRAB_COOLDOWN_MS: u64 = 5000;
static LAST_GRAB_MS: AtomicU64 = AtomicU64::new(0);

/// Minimum time between two accepted DROPs (each one opens VS Code / a tab).
const DROP_COOLDOWN_MS: u64 = 4000;
/// A DROP this soon after a GRAB on the same machine is the hand opening
/// again after the fist, not a real "restore here" — ignore it.
const DROP_AFTER_GRAB_MS: u64 = 3000;
static LAST_DROP_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Returns true (and records "now") when a GRAB is allowed, false while still
/// inside the 5-second cooldown. Shared by both master and slave loops.
fn grab_allowed() -> bool {
    let now = now_ms();
    let last = LAST_GRAB_MS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < GRAB_COOLDOWN_MS {
        return false;
    }
    LAST_GRAB_MS.store(now, Ordering::Relaxed);
    true
}

/// Same idea for DROP. Returns the reason when the DROP must be ignored.
fn drop_blocked() -> Option<&'static str> {
    let now = now_ms();
    if now.saturating_sub(LAST_GRAB_MS.load(Ordering::Relaxed)) < DROP_AFTER_GRAB_MS {
        return Some("just captured on this machine");
    }
    if now.saturating_sub(LAST_DROP_MS.load(Ordering::Relaxed)) < DROP_COOLDOWN_MS {
        return Some("within drop cooldown");
    }
    LAST_DROP_MS.store(now, Ordering::Relaxed);
    None
}

pub async fn run_master(config: Config) -> anyhow::Result<()> {
    let master = MasterNode::new(&config.bind_addr(), &config.password);

    let ipc = IpcListener::bind(config.ipc_port).await?;
    let master_for_ipc = master.clone();
    spawn_local_report_logger();
    let node_id = config.node_id.clone();

    tokio::spawn(async move {
        loop {
            match ipc.recv().await {
                Ok(IpcMessage::Report(r)) => {
                    // The host's own system check: straight into its report log.
                    match Validator::sanitize_report(&r) {
                        Ok(r) => log_device_report(&r),
                        Err(e) => log::debug!("Ignoring invalid local report: {}", e),
                    }
                }
                Ok(IpcMessage::Gesture(packet)) => {
                    log::info!(
                        "Gesture {:?} (confidence: {:.2})",
                        packet.event,
                        packet.confidence
                    );
                    match packet.event {
                        GestureEvent::Grab => {
                            if grab_allowed() {
                                handle_grab_master(&master_for_ipc, &node_id).await;
                            } else {
                                log::debug!("GRAB ignored — within {}s cooldown", GRAB_COOLDOWN_MS / 1000);
                            }
                        }
                        GestureEvent::Drop => {
                            if let Some(why) = drop_blocked() {
                                log::debug!("DROP ignored — {}", why);
                            } else {
                                handle_drop_local(master_for_ipc.get_state()).await;
                            }
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
        Err(e) => log::info!("Context capture failed: {}", e),
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
            restore_snapshot(&snap).await;
        }
        None => {
            log::info!("No state available for DROP");
        }
    }
}

/// Apply a captured snapshot on this machine: clipboard, VS Code file, browser
/// tab — each only if present.
async fn restore_snapshot(snap: &palmsync_shared::StateSnapshot) {
    if !snap.clipboard_content.is_empty() {
        if let Err(e) = crate::clipboard::set_text(&snap.clipboard_content) {
            log::warn!("Clipboard set failed: {}", e);
        }
    }
    if !snap.file_name.is_empty() {
        match executor::prepare_file(snap) {
            Ok(path) => {
                if let Err(e) = executor::open_vscode(&path, snap.cursor_line).await {
                    log::warn!("VS Code open failed: {}", e);
                }
            }
            Err(e) => {
                log::warn!("File restore failed: {}", e);
                notifications::show("PalmSync", &format!("Dosya açılamadı: {}", e));
            }
        }
    }
    if !snap.browser_url.is_empty() {
        if let Err(e) = executor::open_url(&snap.browser_url).await {
            log::warn!("Browser open failed: {}", e);
        }
    }
}

pub async fn run_slave(config: Config) -> anyhow::Result<()> {
    let slave = SlaveNode::new(&config.master_addr, config.master_port, &config.password);

    // Bind IPC first so gestures aren't lost while we connect.
    let ipc = IpcListener::bind(config.ipc_port).await?;
    let reports = spawn_report_forwarder(slave.clone());

    // Connect + keep reconnecting in the background so a master restart or a
    // brief network drop self-heals without losing the session.
    let reconnect_slave = slave.clone();
    let master_addr = config.master_addr.clone();
    tokio::spawn(async move {
        let mut was_connected = false;
        loop {
            if !reconnect_slave.is_connected() {
                match reconnect_slave.connect().await {
                    Ok(resp) => {
                        log::info!("Connected to master: {}", resp.message);
                        if !was_connected {
                            notifications::notify_connected(&master_addr);
                        }
                        was_connected = true;
                    }
                    Err(e) => {
                        if was_connected {
                            log::warn!("Lost connection, reconnecting: {}", e);
                        } else {
                            log::warn!("Connection failed: {} — retrying", e);
                        }
                        was_connected = false;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    });

    log::info!("Slave ready — waiting for gestures on UDP :{}", config.ipc_port);

    loop {
        match ipc.recv().await {
            Ok(IpcMessage::Report(r)) => {
                log::debug!("Forwarding report to host: {}", r.message);
                let _ = reports.send(r);
            }
            Ok(IpcMessage::Gesture(packet)) => {
                log::info!(
                    "Gesture {:?} (confidence: {:.2})",
                    packet.event,
                    packet.confidence
                );
                match packet.event {
                    GestureEvent::Grab => {
                        if grab_allowed() {
                            handle_grab_slave(&slave, &config.node_id).await;
                        } else {
                            log::debug!("GRAB ignored — within {}s cooldown", GRAB_COOLDOWN_MS / 1000);
                        }
                    }
                    GestureEvent::Drop => {
                        if let Some(why) = drop_blocked() {
                            log::debug!("DROP ignored — {}", why);
                        } else {
                            handle_drop_slave(&slave).await;
                        }
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
        Err(e) => log::info!("Context capture failed: {}", e),
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
            restore_snapshot(&snap).await;
        }
        Ok(None) => {
            log::info!("No state available on master");
        }
        Err(e) => {
            log::error!("Drop request failed: {}", e);
        }
    }
}

pub async fn run_idle(_config: Config) -> anyhow::Result<()> {
    log::info!("PalmSync daemon running in idle mode. Waiting for session activation...");
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let new_cfg = Config::load();
        if new_cfg.role == "master" {
            log::info!("Role activated as master, launching master mode");
            return run_master(new_cfg).await;
        } else if new_cfg.role == "slave" {
            log::info!("Role activated as slave, launching slave mode");
            return run_slave(new_cfg).await;
        }
    }
}

