#![feature(async_fn_in_trait)]

pub mod device;
pub mod espnow;
pub mod http;
pub mod storage;
pub mod tcp;
pub mod utils;
pub mod wifi;

use std::time::Duration;

use crate::espnow::EspNowService;
use crate::http::HttpServe;
use crate::storage::StorageService;
use crate::tcp::TcpTransport;
use crate::wifi::WifiService;
use async_executor::LocalExecutor;
use esp_idf_hal::prelude::Peripherals;
use yanet_attributes::AttributesService;
use yanet_broadcast::BroadcastService;
use yanet_core::{ServiceName, Transport};
use yanet_multiplex::MultiplexService;
use yanet_noise::NoiseService;

pub fn run() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();
    let p = Peripherals::take().unwrap();

    //let set_up_pin = PinDriver::input(p.pins.gpio9)?;

    let storage = StorageService::new()?;
    let wifi = WifiService::new(p.modem, &storage)?;

    let espnow = EspNowService::new(&wifi)?;
    let noise = NoiseService::new(|| storage.private_key(&wifi));
    let multiplex = MultiplexService::new();
    let attributes = AttributesService::new(storage.peer_id(&wifi));
    let broadcast = BroadcastService::new();
    let http = HttpServe::new()?;
    let tcp = TcpTransport::new();

    if let Ok(Some(vec)) = storage.get::<Vec<u8>>(attributes.name()) {
        attributes.restore(&vec).ok();
    }

    #[cfg(any(feature = "light-pin13", feature = "light-pin3"))]
    let outdev = {
        use crate::device::OutputDevice;
        use esp_idf_hal::{
            ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution},
            units::Hertz,
        };
        let timer_config = TimerConfig::new()
            .frequency(Hertz(2000))
            .resolution(Resolution::Bits10);
        let timer = LedcTimerDriver::new(p.ledc.timer0, &timer_config).unwrap();
        #[cfg(feature = "light-pin13")]
        let channel = LedcDriver::new(p.ledc.channel0, timer, p.pins.gpio13).unwrap();
        #[cfg(feature = "light-pin3")]
        let channel = LedcDriver::new(p.ledc.channel0, timer, p.pins.gpio3).unwrap();
        OutputDevice::new("led", channel, p.pins.gpio9)
    };

    #[cfg(feature = "button")]
    let button = {
        use device::PushButton;
        PushButton::new("push", p.pins.gpio9)
    };

    let ex = LocalExecutor::new();

    ex.spawn(http.run(&wifi)).detach();
    ex.spawn((&espnow).or(&tcp).then(&noise).handle(&multiplex))
        .detach();
    ex.spawn(multiplex.handle(&broadcast)).detach();
    ex.spawn(multiplex.handle(&attributes)).detach();

    ex.spawn(async {
        loop {
            futures_timer::Delay::new(Duration::from_secs(30)).await;
            let data = attributes.backup();
            storage.set(attributes.name(), &data).ok();
        }
    })
    .detach();

    #[cfg(feature = "button")]
    {
        ex.spawn(button.run_pair_handle(&broadcast)).detach();
        ex.spawn(button.run_push_handle(&attributes)).detach();
    }

    #[cfg(any(feature = "light-pin13", feature = "light-pin3"))]
    {
        ex.spawn(outdev.run_push_handle()).detach();
        ex.spawn(outdev.run_pair_handle(&broadcast, &attributes))
            .detach();
        ex.spawn(outdev.run_listen_handle(&attributes)).detach();
        ex.spawn(outdev.run_blink_handle()).detach();
        ex.spawn(outdev.run_update_handle(&attributes)).detach();
    }

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
