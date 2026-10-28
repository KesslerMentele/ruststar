use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};

struct Session {
    token: String,
    user_id: uuid::Uuid,
    created_at: String,
    expires_at: String,
}



pub fn hash_password(password: &str) -> Result<String, argon2::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt).unwrap();
    assert!(Argon2::default().verify_password(password.as_ref(), &hash).is_ok());
    Ok(hash.to_string())
}

pub fn verify_password(hash: &str, password: &str) -> Result<bool, argon2::Error> {
    let pass_hash = PasswordHash::new(hash).unwrap();
    Ok(Argon2::default().verify_password(password.as_ref(), &pass_hash).is_ok())
}