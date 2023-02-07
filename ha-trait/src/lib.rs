use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct DeviceTrait {
    pub device_name: String,
    pub device_data: DeviceSchema,
}
#[derive(Serialize, Deserialize, Debug)]
#[non_exhaustive]
pub enum DeviceSchema {
    PushButton {
        state_attr: String,
    },
    Light {
        state_attr: String,
    },
    Motion {
        has_motion_attr: String,
    },
    DimmebleLight {
        state_attr: String,
        level_attr: String,
    },
}
