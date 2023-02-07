use crate::database::DB;
use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use sqlx::query;

pub async fn create_account(username: &str, password: &str) -> Result<()> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string();

    query!(
        r#"
            insert into account (account_username, account_password)
            values ($1, $2);
            "#,
        username,
        password_hash,
    )
    .execute(&*DB)
    .await?;
    Ok(())
}
pub async fn valid_password(username: &str, password: &str) -> Result<bool> {
    let db_password = query!(
        r#"select account_password from account
            where account_username = $1
        "#,
        username
    )
    .fetch_one(&*DB)
    .await?
    .account_password;

    let parsed_hash = PasswordHash::new(&db_password).unwrap();
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}
pub async fn change_password(username: &str, password: &str) -> Result<()> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string();

    query!(
        r#"update account
            set account_password = $2
            where account_username = $1"#,
        username,
        password_hash
    )
    .execute(&*DB)
    .await?;
    Ok(())
}
