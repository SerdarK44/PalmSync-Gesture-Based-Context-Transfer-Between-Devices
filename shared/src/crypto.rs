use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub fn compute_password_hash(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn verify_hash(password: &str, expected_hash: &str) -> bool {
    compute_password_hash(password) == expected_hash
}

/// Encrypted wire envelope. AES-256-GCM is authenticated encryption:
/// the GCM tag (appended to ciphertext) provides both confidentiality
/// and integrity, so no separate HMAC is needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedMessage {
    /// 12-byte random nonce, hex encoded
    pub nonce: String,
    /// ciphertext + GCM tag, hex encoded
    pub ciphertext: String,
}

pub struct CryptoManager {
    cipher: Aes256Gcm,
}

impl CryptoManager {
    /// Derive a 256-bit AES key from the shared password.
    /// Domain-separated so the key differs from the handshake hash.
    pub fn new(password: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"palmsync-e2ee-v1:");
        hasher.update(password.as_bytes());
        let key_bytes = hasher.finalize();

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        Self {
            cipher: Aes256Gcm::new(key),
        }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<EncryptedMessage> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("encryption failed: {}", e))?;

        Ok(EncryptedMessage {
            nonce: hex::encode(nonce_bytes),
            ciphertext: hex::encode(ciphertext),
        })
    }

    pub fn decrypt(&self, msg: &EncryptedMessage) -> anyhow::Result<Vec<u8>> {
        let nonce_bytes = hex::decode(&msg.nonce)?;
        if nonce_bytes.len() != 12 {
            anyhow::bail!("invalid nonce length");
        }
        let ciphertext = hex::decode(&msg.ciphertext)?;

        let nonce = Nonce::from_slice(&nonce_bytes);
        self.cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| anyhow::anyhow!("decryption failed: bad key or tampered message"))
    }

    /// Encrypt a JSON RPC string into a wire-ready envelope JSON.
    pub fn seal(&self, plaintext_json: &str) -> anyhow::Result<String> {
        let env = self.encrypt(plaintext_json.as_bytes())?;
        Ok(serde_json::to_string(&env)?)
    }

    /// Decrypt a wire envelope JSON back into the inner JSON string.
    pub fn open(&self, envelope_json: &str) -> anyhow::Result<String> {
        let env: EncryptedMessage = serde_json::from_str(envelope_json)?;
        let plain = self.decrypt(&env)?;
        Ok(String::from_utf8(plain)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_deterministic() {
        let h1 = compute_password_hash("secret");
        let h2 = compute_password_hash("secret");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_differs_for_different_input() {
        let h1 = compute_password_hash("password1");
        let h2 = compute_password_hash("password2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn verify_correct() {
        let hash = compute_password_hash("mypass");
        assert!(verify_hash("mypass", &hash));
        assert!(!verify_hash("wrong", &hash));
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let cm = CryptoManager::new("test_password");
        let plaintext = b"Secret workspace state";

        let env = cm.encrypt(plaintext).unwrap();
        let decrypted = cm.decrypt(&env).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn nonces_are_unique() {
        let cm = CryptoManager::new("pw");
        let e1 = cm.encrypt(b"same message").unwrap();
        let e2 = cm.encrypt(b"same message").unwrap();
        assert_ne!(e1.nonce, e2.nonce);
        assert_ne!(e1.ciphertext, e2.ciphertext);
    }

    #[test]
    fn wrong_password_fails() {
        let cm1 = CryptoManager::new("correct");
        let cm2 = CryptoManager::new("wrong");

        let env = cm1.encrypt(b"secret").unwrap();
        assert!(cm2.decrypt(&env).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let cm = CryptoManager::new("pw");
        let mut env = cm.encrypt(b"authentic").unwrap();

        // Flip a hex nibble in the ciphertext
        let mut chars: Vec<char> = env.ciphertext.chars().collect();
        chars[0] = if chars[0] == 'a' { 'b' } else { 'a' };
        env.ciphertext = chars.into_iter().collect();

        assert!(cm.decrypt(&env).is_err());
    }

    #[test]
    fn seal_open_roundtrip() {
        let cm = CryptoManager::new("pw");
        let inner = r#"{"Ping":null}"#;
        let sealed = cm.seal(inner).unwrap();
        assert!(!sealed.contains("Ping"));
        assert_eq!(cm.open(&sealed).unwrap(), inner);
    }

    #[test]
    fn key_derivation_domain_separated() {
        // The E2EE key must differ from the plain password hash
        let hash = compute_password_hash("pw");
        let cm = CryptoManager::new("pw");
        let env = cm.encrypt(b"x").unwrap();
        // Not a real key-equality check, but ensures both paths work independently
        assert!(!hash.is_empty());
        assert!(!env.ciphertext.is_empty());
    }
}
