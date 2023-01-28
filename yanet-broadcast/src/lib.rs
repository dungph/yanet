#![feature(async_fn_in_trait)]

use async_channel::{bounded, Receiver, Sender};
use event_listener::Event;
use serde::{de::DeserializeOwned, Serialize};
use std::{cell::RefCell, collections::BTreeMap};
use yanet_core::{authenticate::PeerId, Authenticated, Channel, Service, ServiceName};

type Pair<T> = (Sender<T>, Receiver<T>);

pub struct BroadcastService {
    peers: RefCell<BTreeMap<PeerId, Sender<Vec<u8>>>>,
    buf: RefCell<(PeerId, Vec<u8>)>,
    listener: Event,
}

impl BroadcastService {
    pub fn new() -> Self {
        Self {
            peers: RefCell::new(BTreeMap::new()),
            buf: RefCell::new((PeerId::from([0; 32]), Vec::new())),
            listener: Event::new(),
        }
    }

    pub async fn broadcast(&self, value: &impl Serialize) -> anyhow::Result<()> {
        let data = postcard::to_allocvec(value)?;
        for peer in self.peers.borrow().values().map(|v| v.clone()) {
            peer.send(data.clone()).await.ok();
        }
        Ok(())
    }

    pub async fn listen<T: DeserializeOwned>(&self) -> anyhow::Result<(PeerId, T)> {
        self.listener.listen().await;
        let data = self.buf.borrow();
        let t = postcard::from_bytes(&data.1)?;
        Ok((data.0, t))
    }
}

impl ServiceName for BroadcastService {
    type Name = &'static str;
    fn name(&self) -> Self::Name {
        "messaging"
    }
}
impl<C: Channel + Authenticated> Service<C> for BroadcastService {
    type Output = ();

    async fn upgrade(&self, channel: C) -> anyhow::Result<Self::Output> {
        let peerid = channel.peer_id();
        let (tx, rx) = bounded(10);
        self.peers.borrow_mut().insert(peerid, tx);
        let task1 = async {
            while let Ok(msg) = rx.recv().await {
                channel.send(&msg).await?;
            }
            println!("Done broadcast task1");
            Ok(()) as anyhow::Result<()>
        };
        let task2 = async {
            while let Ok(msg) = channel.recv().await {
                *self.buf.borrow_mut() = (peerid, msg);
                self.listener.notify(usize::max_value());
            }
            println!("Done broadcast task2");
            Ok(())
        };
        futures_lite::future::or(task1, task2).await?;
        println!("Done broadcast");
        Ok(())
    }
}
