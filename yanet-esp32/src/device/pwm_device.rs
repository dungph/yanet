use crate::data_schema::Schema;
use crate::{
    data_schema::{DataSchema, DetailDataSchema},
    storage::{StorageEntry, StorageService},
};
use esp_idf_hal::{
    gpio::OutputPin,
    ledc::{config::TimerConfig, LedcChannel, LedcDriver, LedcTimer, LedcTimerDriver, Resolution},
    peripheral::Peripheral,
    units::Hertz,
};
use futures_lite::future::or;
use serde_json::{Number, Value};
use std::time::Duration;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Clone)]
pub struct PWMDevice<'a> {
    dev: Rc<RefCell<esp_idf_hal::ledc::LedcDriver<'a>>>,
    min: u32,
    max: u32,
    module_title: StorageEntry,
    state_title: StorageEntry,
    duty_title: StorageEntry,
    soft_control: StorageEntry,
    duty: StorageEntry,
    state: StorageEntry,
    name: String,
}

impl<'a> PWMDevice<'a> {
    pub fn new<T: LedcTimer, C: LedcChannel>(
        name: &str,
        timer: impl Peripheral<P = T> + 'a,
        channel: impl Peripheral<P = C> + 'a,
        pin: impl Peripheral<P = impl OutputPin> + 'a,
        storage: StorageService,
    ) -> Self {
        let timer_config = TimerConfig::new()
            .frequency(Hertz(2000))
            .resolution(Resolution::Bits10);
        let timer = LedcTimerDriver::new(timer, &timer_config).unwrap();
        let channel = LedcDriver::new(channel, timer, pin).unwrap();
        let max = channel.get_max_duty();

        let ret = Self {
            min: 0,
            max,
            dev: Rc::new(RefCell::new(channel)),
            module_title: storage.entry(&format!("{name}_module_title")),
            state_title: storage.entry(&format!("{name}_state_title")),
            duty_title: storage.entry(&format!("{name}_duty_title")),
            soft_control: storage.entry(&format!("{name}_soft_control")),
            duty: storage.entry(&format!("{name}_duty")),
            state: storage.entry(&format!("{name}_state")),
            name: name.to_string(),
        };
        let val = ret
            .duty
            .get_or_init(|| serde_json::Value::Number(Number::from(0)));
        ret.state.set(serde_json::Value::Bool(val == 0));

        ret.dev
            .borrow_mut()
            .set_duty(val.as_u64().unwrap() as u32)
            .ok();
        ret
    }
    pub async fn run_handle(&self) {
        let future1 = async {
            loop {
                if let Some(new) = self.duty.wait_new().await.as_u64() {
                    let new = new as u32;
                    let current = self.dev.borrow().get_duty();
                    if new >= self.min && new <= self.max {
                        if let Some(true) = self
                            .soft_control
                            .get_or_init(|| Value::Bool(true))
                            .as_bool()
                        {
                            let mut duties: Vec<u32> = if new >= current {
                                (current..new).collect()
                            } else {
                                (new..current).rev().collect()
                            };

                            let step = duties.len() / 20;
                            if step == 0 {
                            } else {
                                duties.retain(|v| *v as usize % step == 0);
                            }
                            for i in duties {
                                futures_timer::Delay::new(Duration::from_millis(10)).await;
                                self.dev.borrow_mut().set_duty(i).ok();
                            }
                            //if new >= current {
                            //    if new - current > 20 {
                            //        for i in current..new {
                            //            futures_timer::Delay::new(Duration::from_millis(10)).await;
                            //            self.dev.borrow_mut().set_duty(i).ok();
                            //        }
                            //    } else {
                            //    }
                            //    for i in current..new {
                            //        futures_timer::Delay::new(Duration::from_millis(1)).await;
                            //        self.dev.borrow_mut().set_duty(i).ok();
                            //    }
                            //} else {
                            //    for i in (new..current).rev() {
                            //        futures_timer::Delay::new(Duration::from_millis(1)).await;
                            //        self.dev.borrow_mut().set_duty(i).ok();
                            //    }
                            //}
                        }
                        self.dev.borrow_mut().set_duty(new).ok();
                    }
                }
            }
        };
        let future2 = async {
            loop {
                if let Some(new) = self.state.wait_new().await.as_bool() {
                    if new {
                        self.duty.set(Value::Number(self.max.into()));
                    } else {
                        self.duty.set(Value::Number(self.min.into()));
                    }
                }
            }
        };
        or(future1, future2).await
    }
}

impl<'a> Schema for PWMDevice<'a> {
    fn get_schema(&self) -> DataSchema {
        let name = &self.name;
        let onoff_field = DataSchema {
            id: self.state.get_key().to_string(),
            title: self
                .state_title
                .get_or_init(|| Value::String(format!("{name} state")))
                .as_str()
                .map(String::from),
            detail: DetailDataSchema::Bool,
            description: self.state.get().as_bool().map(|b| b.to_string()),
            ..Default::default()
        };
        let level_field = DataSchema {
            id: self.duty.get_key().to_string(),
            title: self
                .duty_title
                .get_or_init(|| Value::String(format!("{name} duty")))
                .as_str()
                .map(String::from),
            detail: DetailDataSchema::Integer {
                minimum: Option::Some(self.min.into()),
                maximum: Option::Some(self.max.into()),
            },
            description: self.duty.get().as_i64().map(|v| v.to_string()),
            ..Default::default()
        };
        let module_title_field = DataSchema {
            id: self.module_title.get_key().to_string(),
            title: Some(format!("{name} title")),
            detail: DetailDataSchema::String,
            ..Default::default()
        };
        let state_title_field = DataSchema {
            id: self.state_title.get_key().to_string(),
            title: Some(format!("{name} state title")),
            detail: DetailDataSchema::String,
            ..Default::default()
        };
        let duty_title_field = DataSchema {
            id: self.duty_title.get_key().to_string(),
            title: Some(format!("{name} duty title")),
            detail: DetailDataSchema::String,
            ..Default::default()
        };
        let soft_control = DataSchema {
            id: self.soft_control.get_key().to_string(),
            title: Some(format!("{name} soft control")),
            detail: DetailDataSchema::Bool,
            ..Default::default()
        };
        let mut settings = BTreeMap::new();
        settings.insert(state_title_field.id.to_string(), state_title_field);
        settings.insert(duty_title_field.id.to_owned(), duty_title_field);
        settings.insert(module_title_field.id.to_owned(), module_title_field);
        settings.insert(soft_control.id.to_owned(), soft_control);
        let settings_schema = DataSchema {
            id: String::from("setting"),
            title: Some(String::from("Setting")),
            detail: DetailDataSchema::Object {
                properties: settings,
            },
            ..Default::default()
        };

        let mut properties = BTreeMap::new();
        properties.insert(onoff_field.id.to_owned(), onoff_field);
        properties.insert(level_field.id.to_owned(), level_field);
        properties.insert(String::from("setting"), settings_schema);
        DataSchema {
            id: self.name.clone(),
            title: self
                .module_title
                .get_or_init(|| Value::String(self.name.clone()))
                .as_str()
                .map(String::from),
            detail: DetailDataSchema::Object { properties },
            ..Default::default()
        }
    }
}
