use std::time::Duration;

use esp_idf_hal::gpio::InputPin;
use yanet_attributes::{AttributesService, Value};
use yanet_broadcast::BroadcastService;

use crate::device::{push_button::PushButton, DeviceSchema};
pub async fn handle<'a, InPin: InputPin>(
    name: &str,
    control_pin: &PushButton<'a, InPin>,
    attr: &AttributesService,
    broad: &BroadcastService,
) {
    let input9 = control_pin;
    let event_field = format!("{}-press", name);
    let pair = async {
        loop {
            input9.wait_push_min(Duration::from_secs(3)).await;
            let msg = DeviceSchema::PushButton {
                name: name.to_owned(),
                state_attr: event_field.to_owned(),
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
