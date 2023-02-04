use std::time::{Duration, Instant};

use esp_idf_hal::{
    gpio::{Input, InputPin, Pin, PinDriver},
    peripheral::Peripheral,
};

pub struct PushButton<'a, P: Pin> {
    normal_high: bool,
    pin: PinDriver<'a, P, Input>,
}

impl<'a, InPin: InputPin + Pin> PushButton<'a, InPin> {
    pub fn normal_high(pin: impl Peripheral<P = InPin> + 'a) -> Self {
        let pin = PinDriver::input(pin).unwrap();
        Self {
            normal_high: true,
            pin,
        }
    }
    pub fn normal_low(pin: impl Peripheral<P = InPin> + 'a) -> Self {
        let pin = PinDriver::input(pin).unwrap();
        Self {
            normal_high: false,
            pin,
        }
    }

    pub fn is_released(&self) -> bool {
        !self.is_pushing()
    }
    pub fn is_pushing(&self) -> bool {
        self.normal_high ^ self.pin.is_high()
    }
    pub async fn wait_push_range(&self, min_dur: Duration, max_dur: Duration) {
        loop {
            while self.is_pushing() {
                futures_timer::Delay::new(Duration::from_millis(50)).await;
            }
            while self.is_released() {
                futures_timer::Delay::new(Duration::from_millis(50)).await;
            }
            let start = Instant::now();
            while self.is_pushing() {
                futures_timer::Delay::new(Duration::from_millis(50)).await;
            }
            let dur = Instant::now().duration_since(start);
            if dur > min_dur && dur < max_dur {
                return;
            }
        }
    }
    pub async fn wait_push_min(&self, min_dur: Duration) {
        loop {
            while self.is_pushing() {
                futures_timer::Delay::new(Duration::from_millis(50)).await;
            }
            while self.is_released() {
                futures_timer::Delay::new(Duration::from_millis(80)).await;
            }
            let start = Instant::now();
            while self.is_pushing() {
                futures_timer::Delay::new(Duration::from_millis(80)).await;
                if Instant::now().duration_since(start) > min_dur {
                    return;
                }
            }
        }
    }
}
