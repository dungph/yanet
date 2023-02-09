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
    Begin,
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
                    match data.payload {
                        Packet::Ping => {
                            println!("Recv Ping");
                            this2.send_pong(addr).unwrap();
                        }
                        Packet::Pong => {
                            println!("Recv Pong");
                            if dbg!(handlers.get(&addr).map(|s| !s.is_closed()).unwrap_or(false)) {
                            } else {
                                let (tx, rx) = unbounded();
                                handlers.insert(addr, tx);
                                dbg!(this2.incoming.0.try_send(chan(rx))).ok();
                                this2.send_begin(addr).unwrap();
                            }
                        }
                        Packet::Begin => {
                            println!("recv begin");
                            let (tx, rx) = unbounded();
                            handlers.insert(addr, tx);
                            dbg!(this2.incoming.0.try_send(chan(rx))).ok();
                        }
                        Packet::Message(msg) => {
                            handlers.get(&addr).map(|s| s.try_send(msg).ok());
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
        println!("Send ping ");
        self.send_packet(None, Packet::Ping)
    }
    fn send_pong(&self, addr: [u8; 6]) -> Result<()> {
        println!("Send pong");
        self.send_packet(Some(addr), Packet::Pong)
    }
    fn send_begin(&self, addr: [u8; 6]) -> Result<()> {
        self.send_packet(Some(addr), Packet::Begin)
    }
    fn send_payload(&self, addr: [u8; 6], payload: &[u8]) -> Result<()> {
        self.send_packet(Some(addr), Packet::Message(payload))?;
        Ok(())
    }
    pub async fn advertise(&self) -> Result<()> {
        self.send_ping()?;

        Ok(())
    }
}

impl Transport for EspNowService {
    type Channel = EspNowChannel;
    async fn get(&self) -> Result<Option<<EspNowService as Transport>::Channel>> {
        loop {
            println!("advertise");
            self.advertise().await?;
            if let Some(ret) = self.incoming.1.recv().timeout_secs(10).await.transpose()? {
                println!("new con");
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
        let recv = self.rx.recv().await?;
        println!("Received {} bytes", recv.len());
        Ok(recv)
    }
    async fn send(&self, data: &[u8]) -> Result<()> {
        println!("Sending {} bytes", data.len());
        self.espnow.send_payload(self.addr, data)?;
        futures_timer::Delay::new(Duration::from_millis(10)).await;
        Ok(())
    }
}
