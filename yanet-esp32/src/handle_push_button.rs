use std::time::Duration;

use esp_idf_hal::gpio::InputPin;
use ha_trait::{DeviceSchema, DeviceTrait};
use yanet_attributes::{AttributesService, Value};
use yanet_broadcast::BroadcastService;

use crate::device::push_button::PushButton;
pub async fn handle<'a, InPin: InputPin>(
    name: &str,
    control_pin: &PushButton<'a, InPin>,
    attr: &AttributesService,
    broad: &BroadcastService,
) {
    let input9 = control_pin;
    let event_field = format!("{}-press", name);
    broad
        .add_auto_send(&DeviceTrait {
            device_name: name.to_owned(),
            device_data: DeviceSchema::PushButton {
                state_attr: event_field.to_string(),
            },
        })
        .unwrap();
    let pair = async {
        loop {
            input9.wait_push_min(Duration::from_secs(3)).await;
            let msg = DeviceTrait {
                device_name: name.to_owned(),
                device_data: DeviceSchema::PushButton {
                    state_attr: event_field.to_owned(),
                },
            };
            broad.broadcast(&msg).await.ok();
        }
    };
    let update = async {
        loop {
            input9
                .wait_push_range(Duration::from_millis(10), Duration::from_secs(2))
                .await;
            println!("pushed");
            attr.set_attr_and_share(&event_field, Value::Null).await;
        }
    };
    let ex = async_executor::LocalExecutor::new();
    ex.spawn(pair).detach();
    ex.spawn(update).detach();
    loop {
        ex.tick().await;
    }
}
