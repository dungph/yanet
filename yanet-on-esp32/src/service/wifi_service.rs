use anyhow::Result;
use base58::ToBase58;
use embedded_svc::wifi::{
    AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration, Wifi,
};
use esp_idf_hal::modem::Modem;
use esp_idf_svc::{eventloop::EspSystemEventLoop, wifi::EspWifi};
use std::{cell::RefCell, net::Ipv4Addr, rc::Rc, time::Duration};

use super::storage_service::StorageService;

pub fn get_mac() -> [u8; 6] {
    let mut mac = [0u8; 6];
    unsafe {
        esp_idf_sys::esp_wifi_get_mac(0, &mut mac as *mut u8);
    }
    mac
}

#[derive(Clone)]
pub struct WifiService<'a> {
    wifi: Rc<RefCell<EspWifi<'a>>>,
}

impl<'a> WifiService<'a> {
    pub fn new(modem: Modem, _: &StorageService) -> anyhow::Result<Self> {
        let wifi = EspWifi::new(modem, EspSystemEventLoop::take()?, None)?;
        let this = Self {
            wifi: Rc::new(RefCell::new(wifi)),
        };
        this.enable_ap()?;
        this.start()?;
        Ok(this)
    }

    pub fn get_connected(&self) -> Result<Option<String>> {
        match self.wifi.as_ref().borrow().get_configuration()? {
            Configuration::Client(sta) => Ok(Some(sta.ssid.to_string())),
            Configuration::Mixed(sta, _) => Ok(Some(sta.ssid.to_string())),
            _ => Ok(None),
        }
    }
    pub fn get_ip(&self) -> Result<Ipv4Addr> {
        Ok(self.wifi.borrow().sta_netif().get_ip_info()?.ip)
    }
    pub fn start(&self) -> Result<()> {
        self.wifi.as_ref().borrow_mut().start()?;
        Ok(())
    }
    pub fn is_started(&self) -> Result<bool> {
        Ok(self.wifi.borrow().is_started()?)
    }
    pub async fn wait_start(&self) -> Result<()> {
        while !self.is_started()? {
            futures_timer::Delay::new(Duration::from_millis(100)).await;
        }
        Ok(())
    }

    fn default_ap_conf() -> AccessPointConfiguration {
        AccessPointConfiguration {
            ssid: "ESP32".into(),
            ssid_hidden: true,
            auth_method: AuthMethod::None,
            max_connections: 0,
            channel: 1,
            ..Default::default()
        }
    }
    fn public_ap_conf() -> AccessPointConfiguration {
        AccessPointConfiguration {
            ssid: format!("ESP32-{}", get_mac().to_base58()).as_str().into(),
            ssid_hidden: false,
            auth_method: AuthMethod::None,
            max_connections: 5,
            channel: 1,
            ..Default::default()
        }
    }
    pub fn enable_ap(&self) -> Result<()> {
        let mut wifi = self.wifi.as_ref().borrow_mut();
        let conf = wifi.get_configuration()?;
        let conf = match conf {
            Configuration::None | Configuration::AccessPoint(_) => {
                Configuration::AccessPoint(Self::public_ap_conf())
            }
            Configuration::Client(sta_conf) | Configuration::Mixed(sta_conf, _) => {
                Configuration::Mixed(sta_conf, Self::public_ap_conf())
            }
        };
        wifi.set_configuration(&conf)?;
        Ok(())
    }
    pub fn disable_ap(&self) -> Result<()> {
        let mut wifi = self.wifi.as_ref().borrow_mut();
        let conf = wifi.get_configuration()?;
        let conf = match conf {
            Configuration::None | Configuration::AccessPoint(_) => {
                Configuration::AccessPoint(Self::default_ap_conf())
            }
            Configuration::Client(sta_conf) | Configuration::Mixed(sta_conf, _) => {
                Configuration::Mixed(sta_conf, Self::default_ap_conf())
            }
        };
        wifi.set_configuration(&conf)?;
        Ok(())
    }
    pub async fn connect(&self, ssid: &str, pwd: &str) -> Result<()> {
        {
            let mut wifi = self.wifi.borrow_mut();
            let conf = wifi.get_configuration()?;
            let sta_conf = ClientConfiguration {
                ssid: ssid.into(),
                password: pwd.into(),
                auth_method: if pwd.is_empty() {
                    AuthMethod::None
                } else {
                    AuthMethod::WPA2Personal
                },
                ..Default::default()
            };
            let conf = match conf {
                Configuration::None | Configuration::Client(_) => Configuration::Client(sta_conf),
                Configuration::AccessPoint(ap_conf) | Configuration::Mixed(_, ap_conf) => {
                    Configuration::Mixed(sta_conf, ap_conf)
                }
            };
            wifi.set_configuration(&conf)?;
            wifi.start()?;
        }
        self.wait_start().await?;
        self.wifi.as_ref().borrow_mut().connect()?;
        Ok(())
    }

    pub fn disconnect(&self) -> Result<()> {
        self.wifi.borrow_mut().disconnect()?;
        Ok(())
    }
    pub fn is_connected(&self) -> Result<bool> {
        Ok(self.wifi.borrow_mut().is_connected()?)
    }

    pub async fn wait_connect(&self) -> Result<()> {
        loop {
            if self.is_connected()? && self.get_ip()? != Ipv4Addr::new(0, 0, 0, 0) {
                break Ok(());
            }
            futures_timer::Delay::new(Duration::from_millis(100)).await
        }
    }
    pub fn active_interface(&self) -> u32 {
        esp_idf_sys::esp_interface_t_ESP_IF_WIFI_AP
    }
}
