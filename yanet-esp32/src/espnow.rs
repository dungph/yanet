use anyhow::Result;
use async_channel::{bounded, unbounded, Receiver, Sender};
use esp_idf_svc::espnow::{EspNow, BROADCAST};
use esp_idf_sys::esp_now_peer_info;
use esp_idf_sys::esp_wifi_get_mac;
use future_utils::FutureTimeout;
use postcard::to_allocvec;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use yanet_core::{Channel, Transport};

use crate::wifi::WifiService;

#[derive(Serialize, Deserialize, Debug)]
struct RawPacket<P = Vec<u8>> {
    destination: Option<[u8; 6]>,
    payload: Packet<P>,
}

#[derive(Serialize, Deserialize, Debug)]
enum Packet<P = Vec<u8>> {
    Ping,
    Pong,
    Message(P),
}

type Pair<T> = (Sender<T>, Receiver<T>);

#[derive(Clone)]
pub struct EspNowService {
    mac: [u8; 6],
    espnow: Arc<EspNow>,
    incoming: Pair<EspNowChannel>,
}

impl EspNowService {
    pub fn new(_wifi: &WifiService<'_>) -> Result<Self> {
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
                    let sender = handlers
                        .entry(addr)
                        .and_modify(|s| {
                            if s.is_closed() {
                                let (tx, rx) = unbounded();
                                this2.incoming.0.try_send(chan(rx));
                                *s = tx;
                            }
                        })
                        .or_insert_with(|| {
                            let (tx, rx) = unbounded();
                            this2.incoming.0.try_send(chan(rx));
                            tx
                        })
                        .clone();
                    match data.payload {
                        Packet::Ping => {
                            println!("Ping");
                            this2.send_pong(addr).ok();
                        }
                        Packet::Pong => {
                            println!("Pong");
                        }
                        Packet::Message(msg) => {
                            if data.destination == Some(mac) {
                                sender.try_send(msg).ok();
                            }
                        }
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
        println!("Send ping");
        self.send_packet(None, Packet::Ping)
    }
    fn send_pong(&self, addr: [u8; 6]) -> Result<()> {
        println!("Send pong ");
        self.send_packet(Some(addr), Packet::Pong)
    }
    fn send_payload(&self, addr: [u8; 6], payload: &[u8]) -> Result<()> {
        println!("espnow sending {}", payload.len());
        self.send_packet(Some(addr), Packet::Message(payload))?;
        Ok(())
    }
    pub fn advertise(&self) -> Result<()> {
        self.send_ping()?;
        Ok(())
    }
}

impl Transport for EspNowService {
    type Channel = EspNowChannel;
    async fn get(&self) -> Result<Option<<EspNowService as Transport>::Channel>> {
        loop {
            self.advertise()?;
            if let Some(ret) = self.incoming.1.recv().timeout_secs(10).await.transpose()? {
                break Ok(Some(ret));
            }
        }
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
        let data = self.rx.recv().await?;
        println!("espnow received {}", data.len());
        Ok(data)
    }
    async fn send(&self, data: &[u8]) -> Result<()> {
        self.espnow.send_payload(self.addr, data)?;
        futures_timer::Delay::new(Duration::from_millis(10)).await;
        Ok(())
    }
}
