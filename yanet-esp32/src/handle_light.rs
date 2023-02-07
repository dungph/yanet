use std::time::Duration;

use esp_idf_hal::gpio::InputPin;
use future_utils::FutureTimeout;
use ha_trait::{DeviceSchema, DeviceTrait};
use yanet_attributes::{Action, AttributesService, Value};

use crate::device::{ledc_output::Ledc, push_button::PushButton};

pub async fn ledc_handle<'a, InPin: InputPin>(
    name: &str,
    control_pin: &PushButton<'a, InPin>,
    out_channel: &Ledc<'a>,
    attr: &AttributesService,
    broadcast: &yanet_broadcast::BroadcastService,
) -> () {
    let input9 = control_pin;
    let out12 = out_channel;

    let value_field = format!("{}-value", name);
    broadcast
        .add_auto_send(&DeviceTrait {
            device_name: name.to_owned(),
            device_data: DeviceSchema::Light {
                state_attr: value_field.to_string(),
            },
        })
        .unwrap();
    let push_control = async {
        loop {
            input9
                .wait_push_range(Duration::from_millis(10), Duration::from_secs(2))
                .await;
            out12.toggle_soft().await;
            attr.set_attr_and_share(&value_field, Value::Number(out12.get_duty() as f64))
                .await
        }
    };
    let pair = async {
        loop {
            input9.wait_push_min(Duration::from_secs(4)).await;
            out12.set_blink_period(Some(Duration::from_millis(1500)));
            if let Ok(Some((peer_id, schema))) = broadcast
                .listen::<DeviceTrait>()
                .timeout(Duration::from_secs(20))
                .await
                .transpose()
            {
                match schema.device_data {
                    DeviceSchema::PushButton { state_attr } => {
                        attr.add_action(peer_id, &state_attr, Action::Toggle(value_field.clone()))
                    }
                    _ => {}
                }
            }
            out12.set_blink_period(None);
        }
    };
    let listen = async {
        loop {
            if let Some(Value::Number(v)) = attr.wait(&value_field).await {
                out12.set_duty_soft(v as f32).await;
                attr.set_attr_and_share(&value_field, Value::Number(out12.get_duty() as f64))
                    .await
            }
        }
    };
    let ex = async_executor::LocalExecutor::new();
    ex.spawn(push_control).detach();
    ex.spawn(pair).detach();
    ex.spawn(listen).detach();
    ex.spawn(out12.run_blink_handle()).detach();

    loop {
        ex.tick().await
    }
}
