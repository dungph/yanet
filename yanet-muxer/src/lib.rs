#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

use std::{cell::RefCell, rc::Rc};

use async_channel::{bounded, Receiver, Sender};
use dashmap::DashMap;
use serde::Serialize;

use yanet_core::{Service, Socket};

#[derive(Debug)]
pub enum Error<E> {
    InternalClosed,
    Socket(E),
    Serde(postcard::Error),
}

pub struct Muxer<S: Socket> {
    socket: Rc<RefCell<S>>,
    handlers: Rc<DashMap<String, Sender<(Vec<u8>, S::Addr)>>>,
}

impl<S: Socket> Muxer<S> {
    pub fn new(socket: S) -> Self {
        Self {
            socket: Rc::new(RefCell::new(socket)),
            handlers: Default::default(),
        }
    }

    pub async fn handle<U>(&self, upgrader: U) -> Result<U::Output, U::Error>
    where
        U: Service<MuxerSocket<S>>,
        U::Name: ToString,
    {
        let name = upgrader.name().to_string();
        let (tx, rx) = bounded(10);
        self.handlers.insert(name.clone(), tx);
        let socket = MuxerSocket {
            name,
            socket: self.socket.clone(),
            handlers: self.handlers.clone(),
            receiver: rx,
        };
        upgrader.upgrade(socket).await
    }
}

#[derive(Clone)]
pub struct MuxerSocket<S: Socket> {
    name: String,
    socket: Rc<RefCell<S>>,
    handlers: Rc<DashMap<String, Sender<(Vec<u8>, S::Addr)>>>,
    receiver: Receiver<(Vec<u8>, S::Addr)>,
}

impl<S: Socket> Socket for MuxerSocket<S> {
    type Addr = S::Addr;
    type Error = Error<S::Error>;

    async fn broadcast<D>(&mut self, data: &D) -> Result<(), Self::Error>
    where
        D: Serialize,
    {
        let msg = postcard::to_allocvec(data).unwrap();
        println!("broadcasting {:?}", msg);
        self.socket
            .borrow_mut()
            .broadcast(&(self.name.as_str(), msg))
            .await
            .map_err(Error::Socket)?;
        Ok(())
    }
    async fn send<D>(&mut self, data: &D, addr: Self::Addr) -> Result<(), Self::Error>
    where
        D: Serialize + ?Sized,
    {
        let msg = postcard::to_allocvec(data).map_err(Error::Serde)?;
        println!("sending {:?}", msg);
        self.socket
            .borrow_mut()
            .send(&(self.name.as_str(), msg), addr)
            .await
            .map_err(Error::Socket)?;
        Ok(())
    }

    async fn recv<D>(&mut self) -> Result<(D, Self::Addr), Self::Error>
    where
        D: serde::de::DeserializeOwned,
    {
        let task1 = async {
            loop {
                if let Ok(((name, vec), addr)) =
                    self.socket.borrow_mut().recv::<(String, Vec<u8>)>().await
                {
                    if let Some(sender) = self.handlers.get(&name) {
                        sender
                            .send((vec, addr))
                            .await
                            .map_err(|_| Error::InternalClosed)?;
                    }
                }
            }
        };
        let task2 = async {
            let (vec, addr) = self
                .receiver
                .recv()
                .await
                .map_err(|_| Error::InternalClosed)?;
            println!("received {:?}", vec);
            let dat = postcard::from_bytes(&vec).map_err(Error::Serde)?;
            Ok((dat, addr))
        };
        futures_micro::or!(task1, task2).await
    }
}
