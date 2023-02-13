use crate::storage::StorageService;
use anyhow::Result;
use async_channel::bounded;
use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration, Wifi};
use esp_idf_hal::modem::Modem;
use esp_idf_svc::{eventloop::EspSystemEventLoop, wifi::EspWifi};
use esp_idf_sys::{smartconfig_event_got_ssid_pswd_t, smartconfig_event_t_SC_EVENT_GOT_SSID_PSWD};
use std::{
    cell::RefCell,
    net::Ipv4Addr,
    time::{Duration, Instant},
};

pub struct WifiService<'a> {
    wifi: RefCell<EspWifi<'a>>,
    eventloop: EspSystemEventLoop,
}

impl<'a> WifiService<'a> {
    pub fn new(
        modem: Modem,
        eventloop: EspSystemEventLoop,
        storage: &StorageService,
    ) -> anyhow::Result<Self> {
        let mut wifi = EspWifi::new(modem, eventloop.clone(), None)?;
        wifi.start()?;
        unsafe {
            esp_idf_sys::esp_wifi_set_channel(11, 0);
        }
        let this = Self {
            wifi: RefCell::new(wifi),
            eventloop: eventloop.clone(),
        };

        this.set_conf_stored(storage)?;
        Ok(this)
    }

    pub async fn smartconfig(&self, storage: &StorageService) -> Result<(String, String)> {
        unsafe {
            esp_idf_sys::esp_smartconfig_set_type(
                esp_idf_sys::smartconfig_type_t_SC_TYPE_ESPTOUCH_V2,
            );
            let config = esp_idf_sys::smartconfig_start_config_t {
                enable_log: false,
                esp_touch_v2_enable_crypt: false,
                esp_touch_v2_key: std::ptr::null_mut(),
            };
            esp_idf_sys::esp_smartconfig_start(&config);
        }

        let (tx, rx) = bounded(5);

        let sub = unsafe {
            self.eventloop.subscribe_raw(
                esp_idf_sys::SC_EVENT,
                esp_idf_sys::ESP_EVENT_ANY_ID,
                move |event_data| {
                    if event_data.event_id == smartconfig_event_t_SC_EVENT_GOT_SSID_PSWD as i32 {
                        let data = event_data.as_payload::<smartconfig_event_got_ssid_pswd_t>();
                        let ssid =
                            String::from_utf8_lossy(data.ssid.split(|b| *b == 0).next().unwrap())
                                .to_string();
                        let pwd = String::from_utf8_lossy(
                            data.password.split(|b| *b == 0).next().unwrap(),
                        )
                        .to_string();
                        tx.try_send((ssid, pwd)).ok();
                    }
                },
            )?
        };

        let (ssid, pass) = rx.recv().await?;

        unsafe {
            esp_idf_sys::esp_smartconfig_stop();
        }
        drop(sub);
        self.set_conf(&ssid, &pass)?;
        storage.set("wifi_ssid", &ssid)?;
        storage.set("wifi_pass", &pass)?;
        Ok((ssid, pass))
    }

    pub fn set_conf_stored(&self, storage: &StorageService) -> Result<()> {
        let ssid: String = storage.get("wifi_ssid")?.unwrap_or("public".into());
        let password: String = storage.get("wifi_pass")?.unwrap_or("".into());
        self.set_conf(&ssid, &password)?;
        Ok(())
    }

    pub fn has_stored(&self, storage: &StorageService) -> Result<bool> {
        Ok(storage.get::<String>("wifi_ssid")?.is_some())
    }
    pub fn set_conf(&self, ssid: &str, pwd: &str) -> Result<()> {
        let conf = Configuration::Client(ClientConfiguration {
            ssid: ssid.into(),
            password: pwd.into(),
            auth_method: if pwd.is_empty() {
                AuthMethod::None
            } else {
                AuthMethod::WPA2Personal
            },
            ..Default::default()
        });
        self.wifi.borrow_mut().set_configuration(&conf)?;
        self.wifi.borrow_mut().start()?;
        Ok(())
    }

    pub async fn connect(&self, retry: Duration) -> Result<()> {
        let mut start = Instant::now();
        self.wifi.borrow_mut().connect()?;
        loop {
            if start.elapsed() >= retry {
                start = Instant::now();
                self.wifi.borrow_mut().connect();
            }
            if self.is_connected()?
                && self.wifi.borrow().sta_netif().get_ip_info()?.ip != Ipv4Addr::new(0, 0, 0, 0)
            {
                break;
            }
            futures_timer::Delay::new(Duration::from_millis(100)).await;
        }
        Ok(())
    }

    pub fn is_connected(&self) -> Result<bool> {
        Ok(self.wifi.borrow().is_connected()?)
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.wifi.borrow_mut().disconnect()?;
        Ok(())
    }

    pub(crate) async fn wait_disconnect(&self) -> Result<()> {
        loop {
            if !self.is_connected()? {
                break Ok(());
            }
            futures_timer::Delay::new(Duration::from_millis(100)).await;
        }
    }
}

//#[derive(Serialize, Deserialize, Debug)]
//enum Msg {
//    Online,
//}
//impl ServiceName for WifiService<'_> {
//    type Name = &'static str;
//
//    fn name(&self) -> Self::Name {
//        "wifi"
//    }
//}
//
//impl<C: Channel> Service<C> for WifiService<'_> {
//    type Output = ();
//
//    async fn upgrade(&self, channel: C) -> anyhow::Result<<WifiService<'_> as Service<C>>::Output> {
//        let mut remote_online = false;
//
//        let _dropper = Dropper::new(|| {
//            *self.online_counter.borrow_mut() -= 1;
//        });
//
//        let task1 = async {
//            loop {
//                if self.is_connected()? || *self.online_counter.borrow() > 0 {
//                    channel.send_postcard(&Msg::Online).await?;
//                }
//                futures_timer::Delay::new(Duration::from_secs(4)).await;
//            }
//        };
//
//        let task2 = async {
//            loop {
//                while channel
//                    .recv_postcard::<Msg>()
//                    .timeout_secs(5)
//                    .await
//                    .transpose()?
//                    .is_none()
//                {}
//                *self.online_counter.borrow_mut() += 1;
//                while channel
//                    .recv_postcard::<Msg>()
//                    .timeout_secs(5)
//                    .await
//                    .transpose()?
//                    .is_some()
//                {}
//                *self.online_counter.borrow_mut() -= 1;
//            }
//        };
//        futures_lite::future::or(task1, task2).await
//    }
//}
//
//pub struct Dropper<F: FnOnce()> {
//    f: F,
//}
//
//impl<F: FnOnce()> Dropper<F> {
//    pub fn new(f: F) -> Self {
//        Self { f }
//    }
//}
//impl<F: FnOnce()> Drop for Dropper<F> {
//    fn drop(&mut self) {
//        (self.f)()
//    }
//}
