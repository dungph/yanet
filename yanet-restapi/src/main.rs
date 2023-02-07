use base58::ToBase58;
use ha_trait::DeviceTrait;
use yanet_attributes::AttributesService;
use yanet_broadcast::BroadcastService;
use yanet_core::Transport;
use yanet_multiplex::MultiplexService;
use yanet_noise::NoiseService;
use yanet_tcp::TcpTransport;

pub mod api;
pub mod database;
pub mod yanet;

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    database::migrate().await?;
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
    server.at("/api/peer/new").post(crate::api::peer::new_peer);
    server
        .at("/api/device/name/all")
        .post(api::device::all_device);
    server.at("/api/device/data").post(api::device::device_data);
    server
        .at("/api/attribute/data")
        .post(api::attribute::get_attribute);
    server
        .at("/api/attribute/data/new")
        .post(api::attribute::set_attribute);

    async_std::task::spawn_local(async {
        let key = rand::random::<[u8; 32]>();
        let tcp = TcpTransport::new();
        let noise = NoiseService::new(|| key);
        let multiplex = MultiplexService::new();
        let broadcast = BroadcastService::new();
        let attributes = AttributesService::new(key.into());
        attributes.sync_new_peer(true);
        let recver = attributes.set_recv_any();

        let ex = async_executor::LocalExecutor::new();

        ex.spawn(async {
            loop {
                let peer = noise.next_peer().await;
                database::peer::upsert_peer(&peer.to_base58()).await;
            }
        })
        .detach();
        ex.spawn(async {
            loop {
                let (peer, key, val) = recver.recv().await.unwrap();
                database::attribute::upsert_attribute(&peer.to_base58(), &key, val.into()).await;
            }
        })
        .detach();
        ex.spawn(async {
            while let Ok((peerid, dev)) = broadcast.listen::<DeviceTrait>().await {
                database::device::upsert_device(
                    &peerid.to_base58(),
                    &dev.device_name,
                    serde_json::to_value(&dev.device_data).unwrap(),
                )
                .await
                .ok();
            }
        })
        .detach();
        ex.spawn((&tcp).then(&noise).handle(&multiplex)).detach();
        ex.spawn(multiplex.handle(&attributes)).detach();
        ex.spawn(multiplex.handle(&broadcast)).detach();
        ex.spawn(tcp.listen("0.0.0.0:1234")).detach();
        ex.spawn(async {
            loop {
                println!("Recv\n\n\n\n");
                let (peerid, key, value) = api::attribute::wait_req().await;
                attributes
                    .request_set_attr(&peerid, &key, value.into())
                    .await;
            }
        })
        .detach();
        //ex.spawn()
        loop {
            ex.tick().await
        }
    });

    server.listen("0.0.0.0:8080").await?;
    Ok(())
}
