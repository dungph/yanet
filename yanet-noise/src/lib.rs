#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

use async_channel::{Receiver, Sender};
use async_executor::LocalExecutor;
use serde::{Deserialize, Serialize};
use snow::{HandshakeState, TransportState};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use yanet_core::{ServiceName, Socket};

#[derive(Default)]
pub enum NoiseSession {
    #[default]
    Initiating,
    XX1Sent([u8; 32], Box<HandshakeState>),
    XX2Sent(Box<HandshakeState>),
    Transport(TransportState),
}

impl NoiseSession {
    pub fn get_remote_static(&self) -> Option<[u8; 32]> {
        match self {
            Self::Transport(t) => t.get_remote_static().map(|v| v.try_into().unwrap()),
            _ => None,
        }
    }
    pub fn transport(&mut self) -> Option<&mut TransportState> {
        match self {
            Self::Transport(t) => Some(t),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
enum Msg {
    Hello,
    XX1(Vec<u8>),
    XX2(Vec<u8>),
    XX3(Vec<u8>),
    Payload(u64, Vec<u8>),
}

pub enum Error<E> {
    Noise(snow::Error),
    Io(E),
    Serde(postcard::Error),
}

pub struct NoiseSocket<S: Socket> {
    private_key: [u8; 32],
    socket: S,
    sessions: RefCell<BTreeMap<S::Addr, NoiseSession>>,
}

impl<S: Socket> NoiseSocket<S> {
    pub fn new(private_key: [u8; 32], socket: S) -> Self {
        Self {
            private_key,
            socket,
            sessions: Default::default(),
        }
    }
    pub async fn advertise(&self) -> Result<(), Error<S::Error>> {
        let hello = Msg::Hello;
        let mut buf = [0u8; 8];
        let mut msg = postcard::to_slice(&hello, &mut buf).map_err(Error::Serde)?;
        self.socket.broadcast(&mut msg).await.map_err(Error::Io)?;
        Ok(())
    }
}

impl<S> Socket for NoiseSocket<S>
where
    S: Socket,
    S::Addr: Ord + Clone,
{
    type Addr = [u8; 32];
    type Error = Error<S::Error>;
    async fn broadcast(&self, buf: &[u8]) -> Result<(), Self::Error> {
        let socket_addrs: Vec<S::Addr> = self
            .sessions
            .borrow()
            .iter()
            .filter(|(_a, s)| s.get_remote_static().is_some())
            .map(|(a, _s)| a.clone())
            .collect();
        for addr in socket_addrs {
            buf = vec![0u8; buf.len() + 16];

            //self.socket.send
        }
        Ok(())
    }
    async fn send(&self, buf: &[u8], addr: Self::Addr) -> Result<usize, Error<S::Error>> {
        let socket_addrs: Vec<_> = self
            .sessions
            .borrow()
            .iter()
            .filter(|(_a, s)| s.get_remote_static() == Some(addr))
            .map(|(a, _s)| a)
            .collect();
        Ok(0)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, [u8; 32]), Error<S::Error>> {
        let mut send_buf = [0u8; 128];
        let mut send_len = 0;
        let mut send_addr: Option<S::Addr> = None;
        loop {
            if let Some(addr) = send_addr.take() {
                self.socket
                    .send(&send_buf[..send_len], addr)
                    .await
                    .map_err(Error::Io)?;
            }

            let (len, addr) = self.socket.recv(buf).await.map_err(Error::Io)?;
            let msg = postcard::from_bytes::<Msg>(&buf[..len]).map_err(Error::Serde)?;

            let mut sessions = self.sessions.borrow_mut();
            let entry = sessions.entry(addr.clone()).or_default();
            match (core::mem::take(entry), msg) {
                (NoiseSession::Initiating, Msg::Hello) => {
                    let mut hs = Box::new(builder(true, self.private_key));
                    send_len = hs.write_message(&[], &mut send_buf).map_err(Error::Noise)?;
                    send_addr = Some(addr);
                    let mut xx1 = [0u8; 32];
                    xx1.copy_from_slice(&send_buf[..32]);
                    *entry = NoiseSession::XX1Sent(xx1, hs);
                }
                (NoiseSession::XX1Sent(xx1, _), Msg::XX1(vec)) => {
                    if vec.as_slice() > xx1.as_slice() {
                        let mut hs = Box::new(builder(false, self.private_key));
                        hs.read_message(vec.as_slice(), buf).map_err(Error::Noise)?;
                        send_len = hs.write_message(&[], &mut send_buf).map_err(Error::Noise)?;
                        send_addr = Some(addr);
                        *entry = NoiseSession::XX2Sent(hs);
                    }
                }
                (NoiseSession::XX1Sent(_, mut hs), Msg::XX2(vec)) => {
                    // <- 2
                    hs.read_message(&vec, buf).map_err(Error::Noise)?;
                    // -> 3
                    send_len = hs.write_message(&[], &mut send_buf).map_err(Error::Noise)?;
                    send_addr = Some(addr);
                    let transport = hs.into_transport_mode().map_err(Error::Noise)?;
                    *entry = NoiseSession::Transport(transport);
                }
                (NoiseSession::XX2Sent(mut hs), Msg::XX3(msg)) => {
                    hs.read_message(&msg, buf).map_err(Error::Noise)?;
                    let transport = hs.into_transport_mode().map_err(Error::Noise)?;
                    *entry = NoiseSession::Transport(transport);
                }
                (NoiseSession::Transport(mut t), Msg::Payload(nonce, msg)) => {
                    let len = t.read_message(&msg, buf).map_err(Error::Noise)?;
                    *entry = NoiseSession::Transport(t);
                    return Ok((len, entry.get_remote_static().unwrap()));
                }

                (_, Msg::XX1(msg)) => {
                    let mut hs = Box::new(builder(false, self.private_key));
                    hs.read_message(msg.as_slice(), buf).map_err(Error::Noise)?;
                    send_len = hs.write_message(&[], &mut send_buf).map_err(Error::Noise)?;
                    send_addr = Some(addr);
                    *entry = NoiseSession::XX2Sent(hs);
                }
                (_, Msg::Payload(_, _)) => {
                    let mut hs = Box::new(builder(true, self.private_key));
                    send_len = hs.write_message(&[], &mut send_buf).map_err(Error::Noise)?;
                    send_addr = Some(addr);
                    let mut xx1 = [0u8; 32];
                    xx1.copy_from_slice(&send_buf[..32]);
                    *entry = NoiseSession::XX1Sent(xx1, hs);
                }
                _ => {}
            }
        }
    }
}

fn builder(init: bool, pkey: [u8; 32]) -> HandshakeState {
    let builder = snow::Builder::new("Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap())
        .local_private_key(&pkey);
    if init {
        builder.build_initiator().unwrap()
    } else {
        builder.build_responder().unwrap()
    }
}
