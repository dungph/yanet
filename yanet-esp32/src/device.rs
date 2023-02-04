pub mod ledc_output;
pub mod push_button;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[non_exhaustive]
pub enum DeviceSchema {
    PushButton {
        name: String,
        state_attr: String,
    },
    Light {
        name: String,
        state_attr: String,
    },
    DimmebleLight {
        name: String,
        state_attr: String,
        level_attr: String,
    },
    Wifi {
        ssid_attr: String,
    },
}
