//! Admin-password hashing (Argon2id, PHC string).
//!
//! The hash lives in `server.admin_password_hash` in `config.toml` and is
//! verified against submitted passwords on `/api/v1/login`.

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};

/// Hash `password` with Argon2id and return a PHC-formatted string suitable
/// for storing in config. Returns `Err` only on catastrophic RNG failure.
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("password hash failed: {e}"))
}

/// Verify `password` against a previously stored PHC hash. Returns `Ok(true)`
/// on match, `Ok(false)` on mismatch, and `Err` only when `phc` is malformed.
pub fn verify_password(password: &str, phc: &str) -> Result<bool, String> {
    let parsed =
        PasswordHash::new(phc).map_err(|e| format!("invalid password hash on disk: {e}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let phc = hash_password("hunter2").unwrap();
        assert!(phc.starts_with("$argon2"));
        assert!(verify_password("hunter2", &phc).unwrap());
        assert!(!verify_password("wrong", &phc).unwrap());
    }

    #[test]
    fn each_hash_uses_a_fresh_salt() {
        let a = hash_password("same").unwrap();
        let b = hash_password("same").unwrap();
        assert_ne!(a, b, "salt must differ between hashes");
    }

    #[test]
    fn malformed_phc_reported() {
        assert!(verify_password("x", "not-a-phc-string").is_err());
    }
}
