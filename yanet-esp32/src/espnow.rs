use anyhow::Result;
use async_channel::{bounded, unbounded, Receiver, Sender};
use esp_idf_svc::espnow::{EspNow, BROADCAST};
use esp_idf_sys::esp_now_peer_info;
use esp_idf_sys::esp_wifi_get_channel;
use esp_idf_sys::esp_wifi_get_mac;
use esp_idf_sys::esp_wifi_set_channel;
use postcard::to_allocvec;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Mutex;
use std::time::Instant;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use yanet_core::{Channel, Transport};

use crate::wifi::WifiService;

pub fn set_channel(channel: u8) {
    unsafe {
        esp_wifi_set_channel(channel, 0);
    }
}
pub fn get_channel() -> u8 {
    let mut ret = 0u8;
    let mut sech = 0u32;
    unsafe {
        esp_wifi_get_channel(&mut ret, &mut sech);
    }
    ret
}

#[derive(Serialize, Deserialize, Debug)]
struct RawPacket<P = Vec<u8>> {
    destination: Option<[u8; 6]>,
    payload: Packet<P>,
}

#[derive(Serialize, Deserialize, Debug)]
enum Packet<P = Vec<u8>> {
    Ping { is_online: bool },
    Pong { is_online: bool },
    Begin,
    Message(P),
}

type Pair<T> = (Sender<T>, Receiver<T>);

#[derive(Clone)]
pub struct EspNowService {
    mac: [u8; 6],
    espnow: Arc<EspNow>,
    incoming: Pair<EspNowChannel>,
    is_connected: Arc<AtomicBool>,
    last_online_ping: Arc<Mutex<Instant>>,
}

impl EspNowService {
    pub fn new(wifi: &WifiService<'_>) -> Result<Self> {
        let espnow = Arc::new(EspNow::take()?);
        espnow.add_peer(esp_now_peer_info {
            peer_addr: BROADCAST,
            ..Default::default()
        })?;
        let mut mac = [0; 6];
        unsafe {
            esp_wifi_get_mac(0, &mut mac as *mut u8);
        }
        let this = EspNowService {
            mac,
            espnow: espnow.clone(),
            incoming: bounded(10),
            is_connected: wifi.is_connected.clone(),
            last_online_ping: Arc::new(Mutex::new(Instant::now())),
        };

        let this2 = this.clone();
        let mut handlers: BTreeMap<[u8; 6], Sender<Vec<u8>>> = Default::default();
        espnow.register_recv_cb(move |addr, data| {
            let addr: [u8; 6] = addr.try_into().unwrap();
            let chan = |rx| EspNowChannel {
                espnow: this2.clone(),
                addr,
                rx,
            };
            if let Ok(data) = postcard::from_bytes::<RawPacket>(&data) {
                if data.destination == None || data.destination == Some(mac) {
                    match data.payload {
                        Packet::Ping { is_online } => {
                            if is_online {
                                *this2.last_online_ping.lock().unwrap() = Instant::now();
                            }
                            this2.send_pong(addr).ok();
                        }
                        Packet::Pong { is_online } => {
                            if is_online {
                                *this2.last_online_ping.lock().unwrap() = Instant::now();
                            }
                            if handlers.contains_key(&addr) {
                            } else {
                                let (tx, rx) = unbounded();
                                handlers.insert(addr, tx);
                                this2.incoming.0.try_send(chan(rx)).ok();
                                this2.send_begin(addr).ok();
                            }
                        }
                        Packet::Begin => {
                            let (tx, rx) = unbounded();
                            handlers.insert(addr, tx);
                            this2.incoming.0.try_send(chan(rx)).ok();
                        }
                        Packet::Message(msg) => match handlers.get(&addr) {
                            Some(sender) if !sender.is_closed() => {
                                sender.try_send(msg).ok();
                            }
                            _ => {
                                let (tx, rx) = unbounded();
                                handlers.insert(addr, tx);
                                this2.incoming.0.try_send(chan(rx)).ok();
                            }
                        },
                    }
                }
            }
        })?;
        Ok(this)
    }
    fn send_packet(&self, addr: Option<[u8; 6]>, packet: Packet<&[u8]>) -> Result<()> {
        let data = to_allocvec(&RawPacket {
            destination: addr,
            payload: packet,
        })?;
        self.espnow.send(BROADCAST, &data)?;
        Ok(())
    }
    fn send_ping(&self) -> Result<()> {
        self.send_packet(
            None,
            Packet::Ping {
                is_online: self.is_connected.load(Relaxed),
            },
        )
    }
    fn send_pong(&self, addr: [u8; 6]) -> Result<()> {
        self.send_packet(
            Some(addr),
            Packet::Pong {
                is_online: self.is_connected.load(Relaxed),
            },
        )
    }
    fn send_begin(&self, addr: [u8; 6]) -> Result<()> {
        self.send_packet(Some(addr), Packet::Begin)
    }
    fn send_payload(&self, addr: [u8; 6], payload: &[u8]) -> Result<()> {
        self.send_packet(Some(addr), Packet::Message(payload))?;
        Ok(())
    }
    pub async fn advertise(&self) -> Result<()> {
        for ch in (1..15).rev() {
            if self.is_connected.load(Relaxed) {
                self.send_ping()?;
                return Ok(());
            } else if self.last_online_ping.lock().unwrap().elapsed() < Duration::from_secs(5) {
                self.send_ping()?;
                return Ok(());
            } else {
                set_channel(ch);
                self.send_ping()?;
            }
            futures_timer::Delay::new(Duration::from_millis(100)).await
        }

        Ok(())
    }
}

impl Transport for EspNowService {
    type Channel = EspNowChannel;
    async fn get(&self) -> Result<Option<<EspNowService as Transport>::Channel>> {
        self.advertise().await?;
        Ok(Some(self.incoming.1.recv().await?))
    }
}

#[derive(Clone)]
pub struct EspNowChannel {
    espnow: EspNowService,
    addr: [u8; 6],
    rx: Receiver<Vec<u8>>,
}

impl Channel for EspNowChannel {
    fn is_initiator(&self) -> bool {
        self.espnow.mac > self.addr
    }
    async fn recv(&self) -> Result<Vec<u8>> {
        Ok(self.rx.recv().await?)
    }
    async fn send(&self, data: &[u8]) -> Result<()> {
        self.espnow.send_payload(self.addr, data)?;
        futures_timer::Delay::new(Duration::from_millis(10)).await;
        Ok(())
    }
}
