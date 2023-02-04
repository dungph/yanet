use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
    time::{Duration, Instant},
};

use esp_idf_hal::{
    gpio::OutputPin,
    ledc::{config::TimerConfig, LedcChannel, LedcDriver, LedcTimer, LedcTimerDriver, Resolution},
    peripheral::Peripheral,
    units::Hertz,
};
use event_listener::Event;

pub struct Ledc<'a> {
    old_on_value: RefCell<f32>,
    channel: RefCell<LedcDriver<'a>>,
    blink_period: RefCell<Option<Duration>>,
    change_event: Event,
}

impl<'a> Ledc<'a> {
    pub fn new(
        out_pin: impl Peripheral<P = impl OutputPin> + 'a,
        ledc_timer: impl Peripheral<P = impl LedcTimer> + 'a,
        ledc_channel: impl Peripheral<P = impl LedcChannel> + 'a,
    ) -> Self {
        let timer_config = TimerConfig::new()
            .frequency(Hertz(2000))
            .resolution(Resolution::Bits10);
        let timer = LedcTimerDriver::new(ledc_timer, &timer_config).unwrap();
        let channel = LedcDriver::new(ledc_channel, timer, out_pin).unwrap();
        let old_on_value = if channel.get_duty() == 0 {
            1.0
        } else {
            channel.get_duty() as f32 / channel.get_max_duty() as f32
        };
        Self {
            channel: RefCell::new(channel),
            blink_period: RefCell::new(None),
            old_on_value: RefCell::new(old_on_value),
            change_event: Event::new(),
        }
    }

    fn __set_duty(&self, duty: f32) {
        let mut channel = self.channel.borrow_mut();
        let max = channel.get_max_duty();
        let duty = (duty * max as f32) as u32 % (max + 1);
        channel.set_duty(duty).unwrap();
    }

    pub fn set_duty(&self, duty: f32) {
        let current = self.get_duty();

        if duty == 0.0 && current == 0.0 {
        } else if duty == 0.0 {
            *self.old_on_value.borrow_mut() = current;
        } else {
            *self.old_on_value.borrow_mut() = duty;
        }

        self.__set_duty(duty);
        self.change_event.notify(usize::max_value());
    }
    pub async fn set_duty_soft(&self, duty: f32) {
        static BUSY: AtomicBool = AtomicBool::new(false);
        static EVENT: Event = Event::new();
        if BUSY.compare_exchange(false, true, Relaxed, Relaxed).is_ok() {
        } else {
            EVENT.listen().await
        }

        let current = self.get_duty();

        if duty == 0.0 && current == 0.0 {
        } else if duty == 0.0 {
            *self.old_on_value.borrow_mut() = current;
        } else {
            *self.old_on_value.borrow_mut() = duty;
        }

        let step = (duty - current) / 20.0;

        for i in 1..=20 {
            self.__set_duty(current + step * i as f32);
            futures_timer::Delay::new(Duration::from_millis(10)).await;
        }
        self.__set_duty(duty);
        self.change_event.notify(usize::max_value());
        BUSY.store(false, Relaxed);
        EVENT.notify(1)
    }

    pub fn get_duty(&self) -> f32 {
        let channel = self.channel.borrow_mut();
        let max = channel.get_max_duty();
        let current = channel.get_duty();
        current as f32 / max as f32
    }

    fn get_old_on_duty(&self) -> f32 {
        *self.old_on_value.borrow()
    }

    pub fn toggle(&self) {
        if self.get_duty() > 0.0 {
            self.set_duty(0.0);
        } else {
            let duty = self.get_old_on_duty();
            self.set_duty(duty);
        }
    }
    pub async fn toggle_soft(&self) {
        if self.get_duty() == 0.0 {
            self.set_duty_soft(self.get_old_on_duty()).await;
        } else {
            self.set_duty_soft(0.0).await;
        }
    }

    pub fn set_blink_period(&self, period: Option<Duration>) {
        *self.blink_period.borrow_mut() = period
    }

    pub async fn wait_duty_change(&self) -> f32 {
        self.change_event.listen().await;
        self.get_duty()
    }

    pub async fn run_blink_handle(&self) {
        let mut start = Instant::now();
        loop {
            if let Some(dur) = self.blink_period.borrow().as_ref().cloned() {
                let current = Instant::now();
                if current.duration_since(start) > dur {
                    self.toggle();
                    start = current;
                }
            }
            futures_timer::Delay::new(Duration::from_millis(50)).await;
        }
    }
}
