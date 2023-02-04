pub mod api;
pub mod database;
pub mod yanet;

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    database::migrate().await?;
    yanet::spawn();
    tide::log::start();

    let mut server = tide::new();
    server
        .at("/api/account/new")
        .post(api::account::create_account);
    server
        .at("/api/account/password/valid")
        .post(api::account::valid_password);
    server
        .at("/api/account/password/new")
        .post(api::account::new_password);
    server
        .at("/api/peer/name/all")
        .post(crate::api::peer::all_peer);
    server
        .at("/api/device/name/all")
        .post(api::device::all_device);
    server.at("/api/device/data").post(api::device::device_data);

    server.listen("0.0.0.0:8080").await?;
    Ok(())
}
