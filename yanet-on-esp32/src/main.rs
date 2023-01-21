#![feature(async_fn_in_trait)]

pub mod service;
pub mod utils;

use std::time::Duration;

use async_executor::LocalExecutor;
use base58::ToBase58;
use esp_idf_hal::peripherals::Peripherals;
use service::{espnow_service::EspNowService, storage_service, wifi_service};

use yanet::{
    yanet_multiplex::MultiplexService, yanet_noise::NoiseService, Authenticated, Channel, Named,
    Transport, Upgrade,
};

pub fn run() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();
    let p = Peripherals::take().unwrap();

    let storage = storage_service::StorageService::new()?;
    let wifi = wifi_service::WifiService::new(p.modem, &storage)?;

    let espnow = EspNowService::new(&wifi)?;
    let noise = NoiseService::new(|| storage.private_key(&wifi));
    let multiplex = MultiplexService::new();

    //let channels = EspNowService::new(&wifi)?
    //    .then(NoiseService::new(&storage, &wifi))
    //    .consume_all();

    //channels.get_channel();
    ////let device = SensorDevice::new(p.pins.gpio9);
    //let module1 = PWMDevice::new(
    //    "module-1",
    //    p.ledc.timer0,
    //    p.ledc.channel0,
    //    p.pins.gpio3,
    //    storage.clone(),
    //);
    //let module2 = PWMDevice::new(
    //    "module-2",
    //    p.ledc.timer1,
    //    p.ledc.channel1,
    //    p.pins.gpio4,
    //    storage.clone(),
    //);
    //let controller = Controller::new(
    //    &get_mac().to_base58(),
    //    wifi.clone(),
    //    &storage,
    //    vec![Box::new(module2.clone()), Box::new(module1.clone())],
    //    espnow.clone(),
    //);

    let name = storage.public_key(&wifi).to_base58();

    struct PingService<'a>(&'a str);
    impl<'a> Named for PingService<'a> {
        fn name(&self) -> &str {
            "ping"
        }
    }
    impl<'a, C> Upgrade<C> for PingService<'a>
    where
        C: Channel + Authenticated,
    {
        type Output = ();
        type Error = anyhow::Error;
        async fn upgrade(
            &self,
            channel: C,
        ) -> anyhow::Result<<PingService<'a> as Upgrade<C>>::Output> {
            let task1 = async {
                while let Ok(s) = channel.recv_postcard::<String>().await {
                    println!("{s}");
                }
                Ok(())
            };
            let task2 = async {
                for i in 1.. {
                    let remote = channel.peer_id().to_base58();
                    let name = self.0;
                    channel
                        .send_postcard(&format!("Hello {remote} take {i} from {name}"))
                        .await?;
                    futures_timer::Delay::new(Duration::from_millis(2000)).await;
                }
                Ok(()) as anyhow::Result<()>
            };
            futures_lite::future::or(task1, task2).await?;
            Ok(())
        }
    }
    let ping = PingService(&name);
    let ex = LocalExecutor::new();

    for _ in 0..10 {
        let espnow = &espnow;
        ex.spawn(espnow.then(&noise).then(&multiplex).consume_each())
            .detach();
    }

    for _ in 0..10 {
        ex.spawn(multiplex.handle(&ping)).detach();
    }

    //ex.spawn(wifi.run_handle()).detach();
    //ex.spawn(module1.run_handle()).detach();
    //ex.spawn(module2.run_handle()).detach();
    //ex.spawn(http.run(&controller, &storage)).detach();
    //ex.spawn(espnow.run_handle()).detach();
    //ex.spawn(storage.periodic_store(Duration::from_secs(5)))
    //    .detach();
    //ex.spawn(controller.run_handle()).detach();
    run_executor(ex);
}

fn run_executor(ex: LocalExecutor) -> ! {
    use std::future::Future;
    use std::task::Context;
    let this = std::thread::current();
    let waker = waker_fn::waker_fn(move || {
        this.unpark();
    });
    let mut cx = Context::from_waker(&waker);
    loop {
        while ex.try_tick() {}

        let fut = ex.tick();
        futures_lite::pin!(fut);

        match fut.poll(&mut cx) {
            std::task::Poll::Ready(_) => (),
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}
pub fn main() {
    std::thread::Builder::new()
        .stack_size(40000)
        .name("task_main".to_string())
        .spawn(|| {
            run().ok();
        })
        .unwrap()
        .join()
        .unwrap();
}
