use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

/// Hash a password (Argon2id, salted PHC string).
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

/// Verify a password against a stored Argon2 PHC hash.
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_argon2id_phc() {
        let hash = hash_password("hunter2").unwrap();
        assert!(hash.starts_with("$argon2id$"), "hash={hash}");
    }

    #[test]
    fn verifies_correct_password() {
        let hash = hash_password("hunter2").unwrap();
        assert!(verify_password("hunter2", &hash));
    }

    #[test]
    fn rejects_wrong_password_and_garbage_hash() {
        let hash = hash_password("hunter2").unwrap();
        assert!(!verify_password("wrong", &hash));
        assert!(!verify_password("hunter2", "not-a-valid-hash"));
    }

    #[test]
    fn salts_are_random() {
        assert_ne!(
            hash_password("same").unwrap(),
            hash_password("same").unwrap()
        );
    }
}
