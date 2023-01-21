use anyhow::Result;
use async_channel::{bounded, Receiver, Sender};
use dashmap::DashMap;
use esp_idf_svc::espnow::{EspNow, BROADCAST};
use serde::{Deserialize, Serialize};
use yanet::{Channel, Transport};

use std::{borrow::Borrow, rc::Rc, sync::Arc};

use super::wifi_service::{get_mac, WifiService};

#[derive(Serialize, Deserialize)]
enum EspNowRawMessage<'a> {
    Broadcast,
    Message(&'a [u8]),
}

type Incoming = Receiver<([u8; 6], Receiver<Vec<u8>>)>;

#[derive(Clone)]
pub struct EspNowService {
    espnow: Rc<EspNow>,
    incoming: Incoming,
    handlers: Arc<DashMap<[u8; 6], Sender<Vec<u8>>>>,
}

impl EspNowService {
    pub fn new(wifi: &WifiService) -> anyhow::Result<Self> {
        let interface = wifi.active_interface();
        let espnow = EspNow::take()?;
        espnow.add_peer(esp_idf_sys::esp_now_peer_info {
            peer_addr: BROADCAST,
            ifidx: interface,
            ..Default::default()
        })?;
        let (incoming_tx, incoming) = bounded(10);

        let handlers: Arc<DashMap<[u8; 6], Sender<Vec<u8>>>> = Arc::new(DashMap::new());
        let handlers_ret = handlers.clone();
        espnow.register_recv_cb(move |addr, data| {
            let addr: [u8; 6] = addr.try_into().unwrap();
            handlers
                .entry(addr)
                .and_modify(|sender| {
                    if sender.is_closed() {
                        let (tx, rx) = bounded(10);
                        incoming_tx.try_send((addr, rx)).unwrap();
                        *sender = tx;
                    }
                })
                .or_insert_with(|| {
                    let (tx, rx) = bounded(10);
                    incoming_tx.try_send((addr, rx)).unwrap();
                    tx.clone()
                })
                .try_send(data.to_vec())
                .ok();
        })?;
        Ok(Self {
            espnow: Rc::new(espnow),
            incoming,
            handlers: handlers_ret,
        })
    }
    fn add_peer(&self, addr: [u8; 6]) {
        self.espnow
            .add_peer(esp_idf_sys::esp_now_peer_info {
                peer_addr: addr,
                channel: 0,
                ifidx: 1,
                ..Default::default()
            })
            .ok();
    }
    fn del_peer(&self, addr: [u8; 6]) {
        self.espnow.del_peer(addr).ok();
    }
    pub fn send(&self, addr: [u8; 6], data: &[u8]) -> Result<()> {
        self.espnow.as_ref().borrow().send(addr, data)?;
        Ok(())
    }
    pub async fn find_peer(&self) {}

    pub fn advertise(&self) -> Result<()> {
        self.send(BROADCAST, &postcard::to_allocvec(&(None as Option<()>))?)
    }

    pub async fn next_channel(&self) -> EspNowChannel {
        self.advertise().unwrap();

        let (addr, rx) = self.incoming.recv().await.unwrap();

        self.handlers.retain(|_, s| !s.is_closed());
        self.add_peer(addr);

        println!("new channel");
        EspNowChannel {
            espnow: self.clone(),
            addr,
            rx,
        }
    }
}

impl Transport for EspNowService {
    type Item = EspNowChannel;
    async fn get(&self) -> <EspNowService as Transport>::Item {
        self.next_channel().await
    }
}

#[derive(Clone)]
pub struct EspNowChannel {
    espnow: EspNowService,
    addr: [u8; 6],
    rx: Receiver<Vec<u8>>,
}

impl Channel for EspNowChannel {
    type Error = anyhow::Error;
    fn is_initiator(&self) -> bool {
        get_mac() > self.addr
    }
    async fn recv(&self) -> anyhow::Result<Vec<u8>> {
        loop {
            let recv = self.rx.recv().await?;
            if let Some(vec) = postcard::from_bytes(&recv)? {
                break Ok(vec);
            }
        }
    }
    async fn send(&self, data: &[u8]) -> anyhow::Result<()> {
        self.espnow
            .send(self.addr, &postcard::to_allocvec(&Some(data))?)?;
        Ok(())
    }
}

impl Drop for EspNowChannel {
    fn drop(&mut self) {
        self.espnow.del_peer(self.addr);
    }
}
