use rand::RngCore;
use sha2::{Digest, Sha256};

use palmsync_shared::crypto::compute_password_hash;
use palmsync_shared::{
    HandshakeChallenge, HandshakeInit, HandshakeProof, HandshakeResponse, NodeType, RpcMessage,
};

pub fn build_init(node_id: &str) -> HandshakeInit {
    HandshakeInit {
        node_id: node_id.to_string(),
        node_type: NodeType::Slave,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// 32-byte random challenge nonce, hex encoded. Fresh per connection.
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// proof = SHA-256(password_hash : nonce), where password_hash is the
/// SHA-256 of the shared password. The wire never carries a reusable secret.
pub fn compute_proof(password: &str, nonce: &str) -> String {
    proof_from_hash(&compute_password_hash(password), nonce)
}

fn proof_from_hash(password_hash: &str, nonce: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password_hash.as_bytes());
    hasher.update(b":");
    hasher.update(nonce.as_bytes());
    hex::encode(hasher.finalize())
}

/// Constant-time-ish comparison of the received proof against the expected one.
pub fn verify_proof(proof: &str, password_hash: &str, nonce: &str) -> bool {
    let expected = proof_from_hash(password_hash, nonce);
    // Both are fixed-length hex strings; compare byte-wise without early exit.
    if proof.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in proof.bytes().zip(expected.bytes()) {
        diff |= a ^ b;
    }
    diff == 0
}

pub fn wrap_init(init: HandshakeInit) -> String {
    serde_json::to_string(&RpcMessage::HandshakeInit(init)).unwrap()
}

pub fn wrap_challenge(nonce: &str, master_node_id: &str) -> String {
    serde_json::to_string(&RpcMessage::HandshakeChallenge(HandshakeChallenge {
        nonce: nonce.to_string(),
        master_node_id: master_node_id.to_string(),
    }))
    .unwrap()
}

pub fn wrap_proof(node_id: &str, proof: &str) -> String {
    serde_json::to_string(&RpcMessage::HandshakeProof(HandshakeProof {
        node_id: node_id.to_string(),
        proof: proof.to_string(),
    }))
    .unwrap()
}

pub fn make_result(accepted: bool, message: &str, master_node_id: &str) -> String {
    serde_json::to_string(&RpcMessage::HandshakeResult(HandshakeResponse {
        accepted,
        message: message.to_string(),
        master_node_id: master_node_id.to_string(),
    }))
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_matches_for_correct_password() {
        let password = "secret123";
        let hash = compute_password_hash(password);
        let nonce = generate_nonce();

        let proof = compute_proof(password, &nonce);
        assert!(verify_proof(&proof, &hash, &nonce));
    }

    #[test]
    fn proof_fails_for_wrong_password() {
        let hash = compute_password_hash("correct");
        let nonce = generate_nonce();

        let proof = compute_proof("wrong", &nonce);
        assert!(!verify_proof(&proof, &hash, &nonce));
    }

    #[test]
    fn proof_is_nonce_bound_replay_resistant() {
        let password = "secret123";
        let hash = compute_password_hash(password);
        let nonce1 = generate_nonce();
        let nonce2 = generate_nonce();

        // A proof captured for nonce1 must not authenticate under nonce2
        let proof1 = compute_proof(password, &nonce1);
        assert!(verify_proof(&proof1, &hash, &nonce1));
        assert!(!verify_proof(&proof1, &hash, &nonce2));
    }

    #[test]
    fn nonces_are_unique() {
        assert_ne!(generate_nonce(), generate_nonce());
    }
}
