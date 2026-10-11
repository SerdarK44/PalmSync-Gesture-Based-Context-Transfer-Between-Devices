use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    Master,
    Slave,
}

/// Step 1 (slave → master): announce identity. Carries NO secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeInit {
    pub node_id: String,
    pub node_type: NodeType,
    pub version: String,
}

/// Step 2 (master → slave): fresh per-connection random nonce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeChallenge {
    pub nonce: String,
    pub master_node_id: String,
}

/// Step 3 (slave → master): proof = SHA-256(password_hash : nonce).
/// A captured proof is useless on the next connection (new nonce),
/// which closes the replay hole of the old static-hash handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeProof {
    pub node_id: String,
    pub proof: String,
}

/// Step 4 (master → slave): accept / reject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub accepted: bool,
    pub message: String,
    pub master_node_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub project_path: String,
    pub file_name: String,
    pub cursor_line: u32,
    pub clipboard_content: String,
    pub timestamp: u64,
    pub source_node_id: String,
    /// Active browser tab URL at capture time (empty if the foreground app
    /// wasn't a browser). Restored by opening it on the target machine.
    #[serde(default)]
    pub browser_url: String,
    /// UTF-8 contents of the captured file. The other machine usually doesn't
    /// have the file at the same path, so without the contents it could only
    /// open an empty file with the same name.
    #[serde(default)]
    pub file_content: String,
    /// Path of the file relative to `project_path` (e.g. `src/main.rs`), so
    /// the receiver can recreate the folder structure. Empty if unknown.
    #[serde(default)]
    pub file_rel_path: String,
}

/// A problem a machine found (or couldn't fix) about itself — missing file,
/// camera blocked, failed restore... Joined machines send these to the host,
/// so everything ends up in ONE place: the host's `cihaz-raporlari.log`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceReport {
    /// Windows computer name of the machine the report is about.
    pub machine: String,
    /// "error" | "warn" | "info"
    pub level: String,
    /// Which part noticed it: "daemon", "kamera", "sistem", "arayuz"...
    pub source: String,
    pub message: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcMessage {
    HandshakeInit(HandshakeInit),
    HandshakeChallenge(HandshakeChallenge),
    HandshakeProof(HandshakeProof),
    HandshakeResult(HandshakeResponse),
    Grab(StateSnapshot),
    RequestDrop,
    DropResponse(Option<StateSnapshot>),
    Ping,
    Pong,
    NodeJoined { node_id: String },
    NodeLeft { node_id: String },
    Error { code: i32, message: String },
    Report(DeviceReport),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GestureEvent {
    Grab,
    Drop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GesturePacket {
    pub event: GestureEvent,
    pub timestamp_ms: u64,
    pub confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_state_snapshot() {
        let snap = StateSnapshot {
            project_path: "C:\\Users\\test\\project".into(),
            file_name: "main.rs".into(),
            cursor_line: 42,
            clipboard_content: "fn main() {}".into(),
            timestamp: 1234567890,
            source_node_id: "node-1".into(),
            browser_url: String::new(),
            file_content: String::new(),
            file_rel_path: String::new(),
        };
        let json = serde_json::to_string(&snap).unwrap();
        let back: StateSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.cursor_line, 42);
        assert_eq!(back.file_name, "main.rs");
    }

    #[test]
    fn serialize_rpc_message() {
        let msg = RpcMessage::Grab(StateSnapshot {
            project_path: "/path".into(),
            file_name: "lib.rs".into(),
            cursor_line: 10,
            clipboard_content: "data".into(),
            timestamp: 111,
            source_node_id: "n1".into(),
            browser_url: String::new(),
            file_content: String::new(),
            file_rel_path: String::new(),
        });
        let json = serde_json::to_string(&msg).unwrap();
        let back: RpcMessage = serde_json::from_str(&json).unwrap();
        match back {
            RpcMessage::Grab(s) => assert_eq!(s.cursor_line, 10),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn serialize_gesture_packet() {
        let pkt = GesturePacket {
            event: GestureEvent::Grab,
            timestamp_ms: 999,
            confidence: 0.95,
        };
        let json = serde_json::to_string(&pkt).unwrap();
        let back: GesturePacket = serde_json::from_str(&json).unwrap();
        assert_eq!(back.event, GestureEvent::Grab);
    }
}
