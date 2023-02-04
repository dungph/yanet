use anyhow::Result;
use once_cell::sync::Lazy;

pub static DB: Lazy<sqlx::PgPool> = Lazy::new(|| {
    let url = std::env::var("DATABASE_URL").expect("set DATABASE_URL to your postgres uri");
    sqlx::PgPool::connect_lazy(&url).unwrap()
});
pub async fn migrate() -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(&*DB).await?;
    Ok(())
}

pub mod account;
pub mod attribute;
pub mod device;
pub mod peer;
