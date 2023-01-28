use crate::{
    data_schema::{DataSchema, DetailDataSchema, Schema},
    storage::{StorageEntry, StorageService},
};
use embedded_hal::digital::InputPin;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, time::Duration};

pub struct SensorDevice<T: InputPin> {
    title_state: StorageEntry,
    device: Rc<RefCell<T>>,
    state: StorageEntry,
}

impl<T: InputPin> SensorDevice<T> {
    pub fn new(name: &str, device: T, storage: &StorageService) -> Self {
        Self {
            device: Rc::new(RefCell::new(device)),
            title_state: storage.entry(&format!("{name}_title_state")),
            state: storage.entry(&format!("{name}_state")),
        }
    }

    async fn wait_new_state(&self) -> bool {
        let state = self.device.borrow().is_high().unwrap();
        while self.device.borrow().is_high().unwrap() == state {
            futures_timer::Delay::new(Duration::from_millis(50)).await;
        }
        !state
    }
    pub async fn run_handle(&self) {
        loop {
            let state = self.wait_new_state().await;
            self.state.set(serde_json::Value::Bool(state));
        }
    }
}
//impl<T: InputPin> Schema for SensorDevice<T> {
//    fn get_schema(&self) -> DataSchema {
//        let state = DataSchema {
//            id: self.state.get_key().to_string(),
//            title: self.title_state.get().as_str().map(String::from),
//            detail: DetailDataSchema::Bool,
//            ..Default::default()
//        };
//        let edit_title = DataSchema {
//            id: self.title_state.get_key().to_string(),
//            title: Some(String::from("Change state title")),
//            detail: DetailDataSchema::String,
//            ..Default::default()
//        };
//        let mut properties = BTreeMap::new();
//        properties.insert(state.id.to_owned(), state);
//        properties.insert(edit_title.id.to_owned(), edit_title);
//        properties
//    }
//}
