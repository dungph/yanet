use async_executor::LocalExecutor;
use esp_idf_hal::gpio::{Input, Pin};
use futures_lite::Future;
use std::{
    task::Context,
    time::{Duration, Instant},
};

pub async fn wait_long_push<T: Pin>(
    pin: &esp_idf_hal::gpio::PinDriver<'_, T, Input>,
    min: Duration,
) {
    loop {
        while pin.is_high() {
            futures_timer::Delay::new(Duration::from_millis(80)).await;
        }
        let start = Instant::now();
        while pin.is_low() {
            futures_timer::Delay::new(Duration::from_millis(80)).await;
            if Instant::now().duration_since(start) > min {
                return;
            }
        }
    }
}
pub async fn wait_push<T: Pin>(
    pin: &esp_idf_hal::gpio::PinDriver<'_, T, Input>,
    min: Duration,
    max: Duration,
) {
    loop {
        while pin.is_high() {
            futures_timer::Delay::new(Duration::from_millis(80)).await;
        }
        let start = Instant::now();
        while pin.is_low() {
            futures_timer::Delay::new(Duration::from_millis(80)).await;
        }
        let dur = Instant::now().duration_since(start);
        if dur > min && dur < max {
            return;
        }
    }
}

pub fn run_ex(ex: LocalExecutor<'_>) -> ! {
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
