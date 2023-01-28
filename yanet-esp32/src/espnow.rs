use anyhow::Result;
use async_channel::{bounded, Receiver, Sender};
use dashmap::DashMap;
use esp_idf_svc::espnow::{EspNow, BROADCAST};
use esp_idf_sys::{esp_now_peer_info, esp_wifi_get_mac};
use future_utils::FutureTimeout;
use postcard::to_allocvec;
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, rc::Rc, sync::Arc, time::Duration};
use yanet_core::{Channel, Transport};

use crate::wifi::WifiService;

#[derive(Serialize, Deserialize, Debug)]
enum Packet<P: AsRef<[u8]> + Sized> {
    Hello,
    BeginTransport,
    Message(P),
}

type Incoming = Receiver<([u8; 6], Receiver<Packet<Vec<u8>>>)>;

#[derive(Clone)]
pub struct EspNowService {
    mac: [u8; 6],
    espnow: Rc<EspNow>,
    incoming: Incoming,
}

impl EspNowService {
    pub fn new(_: &WifiService) -> anyhow::Result<Self> {
        let espnow = EspNow::take()?;
        espnow.add_peer(esp_now_peer_info {
            peer_addr: BROADCAST,
            ifidx: 1,
            ..Default::default()
        })?;
        let (incoming_tx, incoming) = bounded(1);

        let handlers: Arc<DashMap<[u8; 6], Sender<Packet<Vec<u8>>>>> = Arc::new(DashMap::new());
        let mut my_mac = [0; 6];
        unsafe {
            esp_wifi_get_mac(1, &mut my_mac as *mut u8);
        }
        espnow.register_recv_cb(move |addr, data| {
            let addr: [u8; 6] = addr.try_into().unwrap();
            if let Ok(data) = postcard::from_bytes::<([u8; 6], Packet<Vec<u8>>)>(data) {
                if data.0 == BROADCAST || data.0 == my_mac {
                    let sender = handlers
                        .entry(addr)
                        .and_modify(|sender| {
                            if sender.is_closed() {
                                println!("replace");
                                let (tx, rx) = bounded(5);
                                incoming_tx.try_send((addr, rx)).ok();
                                *sender = tx;
                            }
                        })
                        .or_insert_with(|| {
                            println!("insert new");
                            let (tx, rx) = bounded(5);
                            incoming_tx.try_send((addr, rx)).ok();
                            tx.clone()
                        });
                    if data.0 == my_mac {
                        sender.try_send(data.1).ok();
                    }
                }
            }
        })?;
        Ok(Self {
            espnow: Rc::new(espnow),
            incoming,
            mac: my_mac,
        })
    }

    fn send_hello(&self, addr: [u8; 6]) -> anyhow::Result<()> {
        let data = to_allocvec(&(addr, Packet::<&[u8]>::Hello))?;
        self.espnow.as_ref().borrow().send(BROADCAST, &data)?;
        Ok(())
    }
    fn send_begin(&self, addr: [u8; 6]) -> anyhow::Result<()> {
        let data = to_allocvec(&(addr, Packet::<&[u8]>::BeginTransport))?;
        self.espnow.as_ref().borrow().send(BROADCAST, &data)?;
        Ok(())
    }
    fn send_payload(&self, addr: [u8; 6], payload: &[u8]) -> anyhow::Result<()> {
        let data = to_allocvec(&(addr, Packet::<&[u8]>::Message(payload)))?;
        self.espnow.as_ref().borrow().send(BROADCAST, &data)?;
        Ok(())
    }
    pub async fn find_peer(&self) {}

    pub fn advertise(&self) -> Result<()> {
        let data = to_allocvec(&(BROADCAST, Packet::<&[u8]>::Hello))?;
        self.espnow.send(BROADCAST, &data)?;
        Ok(())
    }
    async fn get_channel(&self) -> Result<EspNowChannel> {
        println!("advertise");
        self.advertise().unwrap();
        let (addr, rx) = self.incoming.recv().await?;
        self.send_hello(addr)?;
        self.send_hello(addr)?;
        self.send_begin(addr)?;

        let result = async {
            loop {
                let packet = rx.recv().await?;
                if let Packet::Hello = packet {
                    continue;
                }
                if let Packet::BeginTransport = packet {
                    break;
                }
            }
            Ok(()) as anyhow::Result<()>
        }
        .timeout(Duration::from_millis(200))
        .await;
        if result.is_none() {
            anyhow::bail!("Timeout")
        }
        Ok(EspNowChannel {
            espnow: self.clone(),
            addr,
            rx,
            is_init: addr > self.mac,
        })
    }
}

impl Transport for EspNowService {
    type Channel = EspNowChannel;
    async fn get(&self) -> <EspNowService as Transport>::Channel {
        loop {
            let result = self
                .get_channel()
                .timeout(Duration::from_millis(10000))
                .await;
            if let Some(Ok(item)) = result {
                println!("new channel");
                break item;
            }
        }
    }
}

#[derive(Clone)]
pub struct EspNowChannel {
    is_init: bool,
    espnow: EspNowService,
    addr: [u8; 6],
    rx: Receiver<Packet<Vec<u8>>>,
}
impl Channel for EspNowChannel {
    fn is_initiator(&self) -> bool {
        self.is_init
    }
    async fn recv(&self) -> anyhow::Result<Vec<u8>> {
        loop {
            if let Packet::Message(vec) = self.rx.recv().await? {
                break Ok(vec);
            } else {
                self.rx.close();
            }
        }
    }
    async fn send(&self, data: &[u8]) -> anyhow::Result<()> {
        self.espnow.send_payload(self.addr, data)?;
        Ok(())
    }
}
