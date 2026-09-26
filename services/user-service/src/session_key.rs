use rand::Rng;

const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Generate a random 32-character alphanumeric session key (same shape as legacy).
pub fn generate_session_key() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_key_is_32_alphanumeric() {
        let key = generate_session_key();
        assert_eq!(key.len(), 32);
        assert!(key.chars().all(|c| c.is_ascii_alphanumeric()), "key={key}");
    }

    #[test]
    fn session_keys_are_random() {
        assert_ne!(generate_session_key(), generate_session_key());
    }
}
