use sha2::{Digest, Sha256};

pub fn compute_password_hash(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn verify_hash(password: &str, expected_hash: &str) -> bool {
    compute_password_hash(password) == expected_hash
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
}
