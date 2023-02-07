use anyhow::Result;
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use once_cell::sync::Lazy;
use rand::rngs::OsRng;
use sqlx::query;

pub static DB: Lazy<sqlx::PgPool> = Lazy::new(|| {
    let url = std::env::var("DATABASE_URL").expect("set DATABASE_URL to your postgres uri");
    sqlx::PgPool::connect_lazy(&url).unwrap()
});
pub async fn migrate() -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(&*DB).await?;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password("admin".as_bytes(), &salt)
        .unwrap()
        .to_string();
    query!(
        r#"
insert into account(account_username, account_password)
values ('admin', $1)
on conflict(account_username)
do nothing;
        "#,
        password_hash
    )
    .execute(&*DB)
    .await?;
    Ok(())
}

pub mod account;
pub mod attribute;
pub mod device;
pub mod peer;
