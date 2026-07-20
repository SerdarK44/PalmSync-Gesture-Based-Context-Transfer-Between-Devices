use palmsync_shared::crypto::compute_password_hash;
use palmsync_shared::{HandshakeRequest, HandshakeResponse, NodeType, RpcMessage};

pub fn build_handshake_request(node_id: &str, password: &str) -> HandshakeRequest {
    HandshakeRequest {
        node_id: node_id.to_string(),
        node_type: NodeType::Slave,
        password_hash: compute_password_hash(password),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

pub fn verify_handshake(
    request: &HandshakeRequest,
    expected_hash: &str,
    master_node_id: &str,
) -> HandshakeResponse {
    if request.password_hash != expected_hash {
        return HandshakeResponse {
            accepted: false,
            message: "Authentication failed: invalid password".into(),
            master_node_id: master_node_id.to_string(),
        };
    }

    HandshakeResponse {
        accepted: true,
        message: format!("Welcome node {}", request.node_id),
        master_node_id: master_node_id.to_string(),
    }
}

pub fn wrap_handshake(request: HandshakeRequest) -> String {
    serde_json::to_string(&RpcMessage::Handshake(request)).unwrap()
}

pub fn wrap_handshake_response(response: HandshakeResponse) -> String {
    serde_json::to_string(&RpcMessage::HandshakeResult(response)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_accepted_with_correct_password() {
        let password = "secret123";
        let hash = compute_password_hash(password);
        let req = build_handshake_request("slave-1", password);
        let resp = verify_handshake(&req, &hash, "master-1");
        assert!(resp.accepted);
    }

    #[test]
    fn handshake_rejected_with_wrong_password() {
        let hash = compute_password_hash("correct");
        let req = build_handshake_request("slave-1", "wrong");
        let resp = verify_handshake(&req, &hash, "master-1");
        assert!(!resp.accepted);
    }
}
