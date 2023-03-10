#![feature(async_fn_in_trait)]

pub mod device;
pub mod espnow;
pub mod handle_light;
pub mod handle_push_button;
pub mod storage;
pub mod wifi;

use crate::storage::StorageService;
use crate::wifi::WifiService;
use async_executor::LocalExecutor;
use device::{ledc_output::Ledc, push_button::PushButton};
use esp_idf_hal::prelude::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use espnow::EspNowService;
use future_utils::FutureTimeout;
use std::time::Duration;
use yanet_attributes::AttributesService;
use yanet_broadcast::BroadcastService;
use yanet_core::{ServiceName, Transport};
use yanet_multiplex::MultiplexService;
use yanet_noise::NoiseService;
use yanet_tcp::TcpTransport;

pub fn run() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();
    let p = Peripherals::take().unwrap();
    let button9 = PushButton::normal_high(p.pins.gpio9);
    let status_led = {
        #[cfg(not(feature = "pin13"))]
        let ret = Ledc::new(p.pins.gpio3, p.ledc.timer0, p.ledc.channel0);
        #[cfg(feature = "pin13")]
        let ret = Ledc::new(p.pins.gpio13, p.ledc.timer0, p.ledc.channel0);
        ret
    };

    let button0 = PushButton::normal_low(p.pins.gpio0);
    let button1 = PushButton::normal_low(p.pins.gpio1);
    let button6 = PushButton::normal_low(p.pins.gpio6);
    let button7 = PushButton::normal_low(p.pins.gpio7);

    let storage = StorageService::new()?;
    let eventloop = EspSystemEventLoop::take()?;
    let wifi = WifiService::new(p.modem, eventloop.clone(), &storage)?;
    let espnow = EspNowService::new(&wifi)?;
    let tcp = TcpTransport::new();
    let noise = NoiseService::new(|| storage.private_key(&wifi));
    let multiplex = MultiplexService::new();
    let broadcast = BroadcastService::new();
    let attributes = AttributesService::new(storage.peer_id(&wifi));
    if let Ok(Some(vec)) = storage.get::<Vec<u8>>(attributes.name()) {
        attributes.restore(&vec).ok();
    }

    let ex = LocalExecutor::new();

    ex.spawn(
        (&espnow)
            //.or(&tcp)
            .or(&tcp)
            .then(&noise)
            .handle(&multiplex),
    )
    .detach();
    ex.spawn(multiplex.handle(&broadcast)).detach();
    ex.spawn(multiplex.handle(&attributes)).detach();
    dbg!(wifi.set_conf("Nokia", "12346789")).ok();
    ex.spawn(async {
        dbg!(wifi.connect(Duration::from_secs(5)).await).ok();
        dbg!(tcp.connect("192.168.241.23:1234").await);
        wifi.wait_disconnect().await;
    })
    .detach();

    ex.spawn(async {
        loop {
            futures_timer::Delay::new(Duration::from_secs(30)).await;
            let data = attributes.backup();
            storage.set(attributes.name(), &data).ok();
        }
    })
    .detach();

    #[cfg(feature = "ledc")]
    ex.spawn(handle_light::ledc_handle(
        "light",
        &button9,
        &status_led,
        &attributes,
        &broadcast,
    ))
    .detach();

    #[cfg(feature = "button")]
    {
        ex.spawn(handle_push_button::handle(
            "button9",
            &button9,
            &attributes,
            &broadcast,
        ))
        .detach();
        ex.spawn(handle_push_button::handle(
            "button0",
            &button0,
            &attributes,
            &broadcast,
        ))
        .detach();
        ex.spawn(handle_push_button::handle(
            "button1",
            &button1,
            &attributes,
            &broadcast,
        ))
        .detach();
        ex.spawn(handle_push_button::handle(
            "button2",
            &button6,
            &attributes,
            &broadcast,
        ))
        .detach();
        ex.spawn(handle_push_button::handle(
            "button3",
            &button7,
            &attributes,
            &broadcast,
        ))
        .detach();
    }

    ex.spawn(async {
        loop {
            button9.wait_push_min(Duration::from_secs(10)).await;
            status_led.set_blink_period(Some(Duration::from_millis(300)));
            match wifi.smartconfig(&storage).timeout_secs(30).await {
                Some(Ok((_, _))) => {
                    status_led.set_blink_period(Some(Duration::from_millis(1000)));
                    match wifi
                        .connect(Duration::from_secs(100))
                        .timeout_secs(10)
                        .await
                    {
                        Some(Ok(_)) => {
                            status_led.set_blink_period(Some(Duration::from_millis(200)))
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            status_led.set_blink_period(None);
        }
    })
    .detach();
    local_ex::run(async {
        loop {
            ex.tick().await;
        }
    })
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
