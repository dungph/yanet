#![feature(async_fn_in_trait)]

use std::{cell::RefCell, collections::BTreeMap};

use async_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};

use yanet_core::{authenticate::PeerId, Authenticated, Channel, Service, ServiceName};

#[derive(Serialize, Deserialize)]
struct InnerMessage {
    service: Vec<u8>,
    payload: Vec<u8>,
}
pub struct MultiplexService {
    endpoints: RefCell<BTreeMap<Vec<u8>, ChannelPair<MultiplexChannel>>>,
}

impl MultiplexService {
    pub fn new() -> Self {
        Self {
            endpoints: RefCell::new(BTreeMap::new()),
        }
    }

    pub async fn handle<S>(&self, service: &S)
    where
        S: Service<MultiplexChannel>,
        S::Output: std::fmt::Debug,
    {
        let receiver = self
            .endpoints
            .borrow_mut()
            .entry(service.name().as_ref().to_owned())
            .or_insert_with(|| bounded(10))
            .1
            .clone();
        let ex = async_executor::LocalExecutor::new();
        let task1 = async {
            loop {
                if let Ok(channel) = receiver.recv().await {
                    ex.spawn(service.upgrade(channel)).detach();
                }
            }
        };
        let task2 = async {
            loop {
                ex.tick().await;
            }
        };
        futures_lite::future::or(task1, task2).await;
    }
}

type ChannelPair<T> = (Sender<T>, Receiver<T>);
pub struct MultiplexChannel {
    is_init: bool,
    remote_id: PeerId,
    service_name: Vec<u8>,
    tx: Sender<InnerMessage>,
    rx: Receiver<Vec<u8>>,
}
impl ServiceName for MultiplexService {
    type Name = &'static str;
    fn name(&self) -> Self::Name {
        "multiplex"
    }
}
impl<C> Service<C> for MultiplexService
where
    C: Channel + Authenticated,
{
    type Output = ();

    async fn upgrade(&self, channel: C) -> anyhow::Result<Self::Output> {
        let is_init = channel.is_initiator();
        let remote_id = channel.peer_id();
        let (out_tx, out_rx) = bounded::<InnerMessage>(10);
        let receivers: BTreeMap<Vec<u8>, Sender<Vec<u8>>> = self
            .endpoints
            .borrow()
            .iter()
            .map(|(service_name, v)| {
                let (tx, rx) = bounded::<Vec<u8>>(10);
                let channel = MultiplexChannel {
                    is_init,
                    remote_id,
                    service_name: service_name.to_owned(),
                    tx: out_tx.clone(),
                    rx,
                };
                v.0.try_send(channel).ok();
                (service_name.to_owned(), tx)
            })
            .collect();
        let task1 = async {
            while let Ok(msg) = channel.recv_postcard::<InnerMessage>().await {
                if let Some(tx) = receivers.get(&msg.service) {
                    tx.send(msg.payload).await?;
                }
            }
            Ok(()) as anyhow::Result<()>
        };
        let task2 = async {
            while let Ok(msg) = out_rx.recv().await {
                channel.send_postcard(&msg).await?;
            }
            Ok(())
        };
        futures_lite::future::or(task1, task2).await?;
        Ok(())
    }
}

impl Authenticated for MultiplexChannel {
    fn peer_id(&self) -> PeerId {
        self.remote_id
    }
}

impl Channel for MultiplexChannel {
    fn is_initiator(&self) -> bool {
        self.is_init
    }

    async fn recv(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.rx.recv().await?)
    }

    async fn send(&self, buf: &[u8]) -> anyhow::Result<()> {
        let msg = InnerMessage {
            service: self.service_name.to_owned(),
            payload: buf.to_owned(),
        };
        Ok(self.tx.send(msg).await?)
    }
}
