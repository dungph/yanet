pub mod pair_service;

use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

use esp_idf_hal::{
    gpio::{Input, InputPin, Pin, PinDriver},
    ledc::LedcDriver,
};
use serde::{Deserialize, Serialize};
use yanet_attributes::{Action, AttributesService, Value};
use yanet_broadcast::BroadcastService;

use crate::utils::{wait_long_push, wait_push};

#[derive(Serialize, Deserialize, Debug)]
pub enum DeviceSchema {
    PushButton { state_attr: String },
    Switch,
    Light,
    DimmebleLight,
}

pub struct PushButton<'a, P: Pin> {
    pin: PinDriver<'a, P, Input>,
    state_attr: String,
}

impl<'a, P: InputPin + Pin> PushButton<'a, P> {
    pub fn new(name: &str, pin: P) -> Self {
        Self {
            state_attr: format!("{}-state", name),
            pin: PinDriver::input(pin).unwrap(),
        }
    }

    pub async fn run_pair_handle(&self, broadcast: &BroadcastService) -> anyhow::Result<()> {
        loop {
            wait_long_push(&self.pin, Duration::from_secs(3)).await;
            let msg = DeviceSchema::PushButton {
                state_attr: self.state_attr.to_owned(),
            };
            broadcast.broadcast(&msg).await?;
        }
    }

    pub async fn run_push_handle(&self, attributes: &AttributesService) -> anyhow::Result<()> {
        loop {
            wait_push(&self.pin, Duration::from_millis(10), Duration::from_secs(2)).await;
            attributes.upsert(&self.state_attr, Value::Null).await;
        }
    }
}

pub struct OutputDevice<'a, P: Pin> {
    out_pin: RefCell<LedcDriver<'a>>,
    in_pin: PinDriver<'a, P, Input>,
    blink: RefCell<Option<Duration>>,
    state_attr: String,
}

impl<'a, P: InputPin + Pin> OutputDevice<'a, P> {
    pub fn new(name: &str, out_pin: LedcDriver<'a>, in_pin: P) -> Self {
        Self {
            out_pin: RefCell::new(out_pin),
            in_pin: PinDriver::input(in_pin).unwrap(),
            blink: RefCell::new(None),
            state_attr: format!("{}-state", name),
        }
    }

    pub fn get_duty(&self) -> f64 {
        let max = self.out_pin.borrow().get_max_duty();
        self.out_pin.borrow().get_duty() as f64 / max as f64
    }

    pub async fn set_duty_soft(&self, duty: f64) {
        let current = self.get_duty();

        let step = (duty - current) / 20.0;

        for i in 1..=20 {
            self.set_duty(current + step * i as f64);
            futures_timer::Delay::new(Duration::from_millis(10)).await;
        }
        self.set_duty(duty);
    }

    pub fn set_duty(&self, duty: f64) {
        let max = self.out_pin.borrow().get_max_duty();
        let duty = max as f64 * duty;
        self.out_pin
            .borrow_mut()
            .set_duty(max.min(duty as u32))
            .unwrap();
    }

    pub async fn toggle_soft(&self) {
        if self.get_duty() > 0.0 {
            self.set_duty_soft(0.0).await;
        } else {
            self.set_duty_soft(1.0).await;
        }
    }

    pub fn toggle(&self) {
        if self.get_duty() > 0.0 {
            self.set_duty(0.0);
        } else {
            self.set_duty(1.0);
        }
    }

    pub fn blink(&self, duraiton: Option<Duration>) {
        let mut this = self.blink.borrow_mut();
        if let Some(dur) = duraiton {
            this.replace(dur);
        } else {
            this.take();
        }
    }

    pub async fn run_blink_handle(&self) {
        let mut start = Instant::now();
        loop {
            if let Some(dur) = self.blink.borrow().as_ref().cloned() {
                let current = Instant::now();
                if current.duration_since(start) > dur {
                    self.toggle();
                    start = current;
                }
            }
            futures_timer::Delay::new(Duration::from_millis(50)).await;
        }
    }
    pub async fn run_listen_handle(&self, attributes: &AttributesService) -> anyhow::Result<()> {
        loop {
            if let Some(Value::Number(v)) = attributes.wait(&self.state_attr).await {
                self.set_duty(v);
            }
        }
    }

    pub async fn run_update_handle(&self, attributes: &AttributesService) -> anyhow::Result<()> {
        let mut old_state = self.out_pin.borrow().get_duty();
        let max_state = self.out_pin.borrow().get_max_duty() as f64;
        loop {
            let new_state = self.out_pin.borrow().get_duty();
            if old_state != new_state {
                attributes
                    .upsert(
                        &self.state_attr,
                        Value::Number(new_state as f64 / max_state),
                    )
                    .await;
                old_state = new_state;
            }
            futures_timer::Delay::new(Duration::from_millis(200)).await;
        }
    }

    pub async fn run_pair_handle(
        &self,
        broadcast: &BroadcastService,
        attributes: &AttributesService,
    ) -> anyhow::Result<()> {
        loop {
            wait_long_push(&self.in_pin, Duration::from_secs(5)).await;
            println!("Begin pair");
            self.blink(Some(Duration::from_millis(1500)));
            let task1 = async {
                loop {
                    let (peer_id, data) = broadcast.listen::<DeviceSchema>().await?;
                    if let DeviceSchema::PushButton { state_attr } = data {
                        attributes.add_action(
                            peer_id,
                            &state_attr,
                            Action::Toggle(self.state_attr.clone()),
                        );
                        break;
                    }
                }
                Ok(())
            };
            let task2 = async {
                futures_timer::Delay::new(Duration::from_secs(20)).await;
                Ok(()) as anyhow::Result<()>
            };
            futures_lite::future::or(task1, task2).await?;
            self.blink(None);
            println!("End pair");
        }
    }
    pub async fn run_push_handle(&self) -> anyhow::Result<()> {
        loop {
            wait_push(
                &self.in_pin,
                Duration::from_millis(20),
                Duration::from_secs(2),
            )
            .await;
            self.toggle();
            futures_timer::Delay::new(Duration::from_millis(200)).await;
        }
    }
}
