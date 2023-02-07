use yanet_attributes::AttributesService;
use yanet_broadcast::BroadcastService;
use yanet_core::Transport;
use yanet_multiplex::MultiplexService;
use yanet_noise::NoiseService;
use yanet_tcp::TcpTransport;

pub fn spawn() {
    async_std::task::spawn_local(async {
        let key = rand::random::<[u8; 32]>();
        let tcp = TcpTransport::new();
        let noise = NoiseService::new(|| key);
        let multiplex = MultiplexService::new();
        let broadcast = BroadcastService::new();
        let attributes = AttributesService::new(key.into());

        let ex = async_executor::LocalExecutor::new();

        ex.spawn((&tcp).then(&noise).handle(&multiplex)).detach();
        ex.spawn(multiplex.handle(&attributes)).detach();
        ex.spawn(multiplex.handle(&broadcast)).detach();
        ex.spawn(tcp.listen("0.0.0.0:1234")).detach();

        loop {
            ex.tick().await
        }
    });
}
