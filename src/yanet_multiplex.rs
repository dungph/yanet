use std::{cell::RefCell, collections::BTreeMap};

use async_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};

use crate::yanet_core::{
    channel::{Authenticated, Channel},
    upgrade::{Named, Upgrade},
};

#[derive(Serialize, Deserialize)]
struct InnerMessage {
    service: String,
    payload: Vec<u8>,
}
pub struct MultiplexService {
    endpoints: RefCell<BTreeMap<String, ChannelPair<MultiplexChannel>>>,
}

impl MultiplexService {
    pub fn new() -> Self {
        Self {
            endpoints: RefCell::new(BTreeMap::new()),
        }
    }

    pub async fn handle<S>(&self, service: &S)
    where
        S: Upgrade<MultiplexChannel>,
    {
        let receiver = self
            .endpoints
            .borrow_mut()
            .entry(service.name().to_owned())
            .or_insert_with(|| bounded(10))
            .1
            .clone();
        loop {
            if let Ok(channel) = receiver.recv().await {
                service.upgrade(channel).await.ok();
            }
        }
    }
}

type ChannelPair<T> = (Sender<T>, Receiver<T>);
pub struct MultiplexChannel {
    is_init: bool,
    remote_id: [u8; 32],
    service_name: String,
    tx: Sender<InnerMessage>,
    rx: Receiver<Vec<u8>>,
}
impl MultiplexChannel {
    fn new(
        is_init: bool,
        remote_id: [u8; 32],
        service_name: impl AsRef<str>,
        tx: Sender<InnerMessage>,
        rx: Receiver<Vec<u8>>,
    ) -> Self {
        println!("new multiplex channel");
        Self {
            is_init,
            remote_id,
            service_name: service_name.as_ref().to_owned(),
            tx,
            rx,
        }
    }
}
impl Named for MultiplexService {
    fn name(&self) -> &str {
        "multiplex"
    }
}
impl<C> Upgrade<C> for MultiplexService
where
    C: Channel + Authenticated,
{
    type Output = ();
    type Error = anyhow::Error;

    async fn upgrade(&self, channel: C) -> anyhow::Result<Self::Output> {
        println!("upgrade channel");
        let is_init = channel.is_initiator();
        let remote_id = channel.peer_id();
        let (out_tx, out_rx) = bounded::<InnerMessage>(10);
        let receivers: BTreeMap<String, Sender<Vec<u8>>> = self
            .endpoints
            .borrow()
            .iter()
            .map(|(service_name, v)| {
                let (tx, rx) = bounded::<Vec<u8>>(10);
                let channel =
                    MultiplexChannel::new(is_init, remote_id, service_name, out_tx.clone(), rx);
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
    fn peer_id(&self) -> [u8; 32] {
        self.remote_id
    }
}

impl Channel for MultiplexChannel {
    type Error = anyhow::Error;

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
