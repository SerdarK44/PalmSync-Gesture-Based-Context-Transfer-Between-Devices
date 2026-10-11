use palmsync_shared::{RpcMessage, StateSnapshot};

pub fn parse_message(raw: &str) -> Result<RpcMessage, serde_json::Error> {
    serde_json::from_str(raw)
}

pub fn encode_message(msg: &RpcMessage) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

pub fn make_grab(snapshot: StateSnapshot) -> String {
    serde_json::to_string(&RpcMessage::Grab(snapshot)).unwrap()
}

pub fn make_drop_request() -> String {
    serde_json::to_string(&RpcMessage::RequestDrop).unwrap()
}

pub fn make_drop_response(snapshot: Option<StateSnapshot>) -> String {
    serde_json::to_string(&RpcMessage::DropResponse(snapshot)).unwrap()
}

pub fn make_ping() -> String {
    serde_json::to_string(&RpcMessage::Ping).unwrap()
}

pub fn make_pong() -> String {
    serde_json::to_string(&RpcMessage::Pong).unwrap()
}

pub fn make_error(code: i32, message: &str) -> String {
    serde_json::to_string(&RpcMessage::Error {
        code,
        message: message.to_string(),
    })
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_ping() {
        let json = make_ping();
        let msg = parse_message(&json).unwrap();
        assert!(matches!(msg, RpcMessage::Ping));
    }

    #[test]
    fn roundtrip_grab() {
        let snap = StateSnapshot {
            project_path: "/p".into(),
            file_name: "f.rs".into(),
            cursor_line: 7,
            clipboard_content: "x".into(),
            timestamp: 0,
            source_node_id: "n".into(),
            browser_url: String::new(),
            file_content: String::new(),
            file_rel_path: String::new(),
        };
        let json = make_grab(snap);
        let msg = parse_message(&json).unwrap();
        match msg {
            RpcMessage::Grab(s) => assert_eq!(s.cursor_line, 7),
            _ => panic!("expected Grab"),
        }
    }

    #[test]
    fn error_message_roundtrip() {
        let json = make_error(404, "not found");
        let msg = parse_message(&json).unwrap();
        match msg {
            RpcMessage::Error { code, message } => {
                assert_eq!(code, 404);
                assert_eq!(message, "not found");
            }
            _ => panic!("expected Error"),
        }
    }
}
