#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

//use async_channel::{bounded, unbounded, Receiver, Sender};
//use std::{cell::RefCell, collections::BTreeMap};
//use yanet_core::{Authenticated, Channel, Service, ServiceName, Transport};
//
//type Pair<T> = (Sender<T>, Receiver<T>);
//
//enum RelayMessgage {}
//
//pub struct RelayService {
//    connected_peers: RefCell<BTreeMap<[u8; 32], Vec<Pair<Vec<u8>>>>>,
//    request_peers: Pair<[u8; 32]>,
//    incoming: Pair<RelayChannel>,
//}
//
//impl RelayService {
//    pub fn new() -> Self {
//        Self {
//            connected_peers: RefCell::new(BTreeMap::new()),
//            incoming: unbounded(),
//            request_peers: bounded(10),
//        }
//    }
//    pub fn request(&self, peer_id: [u8; 32]) -> anyhow::Result<()> {
//        Ok(self.request_peers.0.try_send(peer_id)?)
//    }
//}
//impl RelayChannel {}
//
//pub struct RelayChannel {
//    tx: Sender<Vec<u8>>,
//    rx: Receiver<Vec<u8>>,
//}
//
//impl ServiceName for RelayService {
//    type Name = &'static str;
//    fn name(&self) -> Self::Name {
//        "relay"
//    }
//}
//
//impl<C: Channel + Authenticated> Service<C> for RelayService {
//    type Output = ();
//
//    async fn upgrade(&self, channel: C) -> anyhow::Result<Self::Output> {
//        let (tx1, rx2) = unbounded();
//        let (tx2, rx1) = unbounded();
//
//        self.connected_peers
//            .borrow_mut()
//            .entry(channel.peer_id())
//            .or_default()
//            .push((tx2, rx2));
//        let task1 = async {
//            while let Ok(value) = channel.recv().await {
//                tx1.send(value).await.ok();
//            }
//            Ok(())
//        };
//        let task2 = async {
//            while let Ok(value) = rx1.recv().await {
//                channel.send(&value).await?;
//            }
//            Ok(()) as anyhow::Result<()>
//        };
//        futures_lite::future::or(task1, task2).await?;
//        Ok(())
//    }
//}
//
//impl Transport for RelayService {
//    type Channel = RelayChannel;
//
//    async fn get(&self) -> Self::Channel {
//        loop {
//            let req = self.request_peers.1.recv().await.unwrap();
//            if let Some(_) = self.connected_peers.borrow().get(&req) {
//                continue;
//            }
//            break;
//        }
//
//        todo!()
//    }
//}
