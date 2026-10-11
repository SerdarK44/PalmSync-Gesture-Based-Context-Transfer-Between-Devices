use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

pub struct AuthManager;

impl AuthManager {
    /// Hash a password with Argon2id (random salt).
    /// Use for at-rest password storage (e.g. config files).
    pub fn hash_password(password: &str) -> anyhow::Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("argon2 hash failed: {}", e))?;

        Ok(hash.to_string())
    }

    /// Verify a password against an Argon2 PHC-format hash.
    pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
        let parsed =
            PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("invalid hash format: {}", e))?;

        match Argon2::default().verify_password(password.as_bytes(), &parsed) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(anyhow::anyhow!("argon2 verify failed: {}", e)),
        }
    }

    /// Generate a 64-char cryptographically secure session token.
    pub fn generate_session_token() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::thread_rng();

        (0..64)
            .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_and_verify() {
        let hash = AuthManager::hash_password("my_secure_password").unwrap();
        assert!(AuthManager::verify_password("my_secure_password", &hash).unwrap());
        assert!(!AuthManager::verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn hashes_are_salted() {
        let h1 = AuthManager::hash_password("same").unwrap();
        let h2 = AuthManager::hash_password("same").unwrap();
        assert_ne!(h1, h2, "salted hashes must differ");
    }

    #[test]
    fn session_tokens_unique() {
        let t1 = AuthManager::generate_session_token();
        let t2 = AuthManager::generate_session_token();
        assert_eq!(t1.len(), 64);
        assert_ne!(t1, t2);
    }

    #[test]
    fn invalid_hash_format_errors() {
        assert!(AuthManager::verify_password("pw", "not-a-phc-hash").is_err());
    }
}
