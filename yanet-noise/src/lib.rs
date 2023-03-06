#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

use serde::{Deserialize, Serialize};
use snow::{HandshakeState, TransportState};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
};
use yanet_core::Socket;

fn builder(init: bool, pkey: [u8; 32]) -> HandshakeState {
    let builder = snow::Builder::new("Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap())
        .local_private_key(&pkey);
    if init {
        builder.build_initiator().unwrap()
    } else {
        builder.build_responder().unwrap()
    }
}

#[derive(Default, Debug)]
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
#[repr(u8)]
enum Msg {
    Hello,
    XX1(Vec<u8>),
    XX2(Vec<u8>),
    XX3(Vec<u8>),
    Payload(u64, Vec<u8>),
}

#[derive(Debug)]
pub enum Error<E> {
    Noise(snow::Error),
    Io(E),
    Serde(postcard::Error),
}

pub struct NoiseSocket<S: Socket> {
    private_key: [u8; 32],
    socket: S,
    sessions: BTreeMap<S::Addr, NoiseSession>,
}

impl<S: Socket> NoiseSocket<S> {
    pub fn new(private_key: [u8; 32], socket: S) -> Self {
        Self {
            private_key,
            socket,
            sessions: Default::default(),
        }
    }
    pub async fn advertise(&mut self) -> Result<(), Error<S::Error>> {
        let hello = Msg::Hello;
        self.socket.broadcast(&hello).await.map_err(Error::Io)?;
        Ok(())
    }
}

impl<S> Socket for NoiseSocket<S>
where
    S: Socket,
    S::Error: Debug,
    S::Addr: Ord + Clone + Debug,
{
    type Addr = [u8; 32];
    type Error = Error<S::Error>;

    async fn broadcast<D>(&mut self, data: &D) -> Result<(), Self::Error>
    where
        D: Serialize,
    {
        let addrs: BTreeSet<[u8; 32]> = self
            .sessions
            .iter()
            .filter_map(|(_a, s)| s.get_remote_static())
            .collect();
        for addr in addrs {
            self.send(data, addr).await?;
        }
        Ok(())
    }

    async fn send<D>(&mut self, data: &D, addr: Self::Addr) -> Result<(), Self::Error>
    where
        D: Serialize + ?Sized,
    {
        let ret = self
            .sessions
            .iter_mut()
            .find_map(|(_a, s)| match s {
                NoiseSession::Transport(t) if t.get_remote_static() == Some(addr.as_slice()) => {
                    return Some((_a, t))
                }
                _ => None,
            })
            .map(|(a, t)| -> Result<_, Error<S::Error>> {
                let mut buf = [0u8; 1024];
                let buf = postcard::to_slice(data, &mut buf).map_err(Error::Serde)?;
                let mut write_buf = [0u8; 1024];
                let nonce = t.sending_nonce();
                let len = t.write_message(buf, &mut write_buf).map_err(Error::Noise)?;
                let msg = Msg::Payload(nonce, write_buf[..len].to_vec());
                Ok((a.clone(), msg))
            })
            .transpose()?;
        if let Some((a, m)) = ret {
            self.socket.send(&m, a).await.map_err(Error::Io)?;
        }
        Ok(())
    }

    async fn recv<D>(&mut self) -> Result<(D, Self::Addr), Self::Error>
    where
        D: serde::de::DeserializeOwned,
    {
        let mut hs_buf = [0u8; 128];
        let mut send_first: Option<(Msg, S::Addr)> = None;

        loop {
            if let Some((data, addr)) = send_first.take() {
                self.socket.send(&data, addr).await.map_err(Error::Io)?;
            }

            let (msg, addr) = self.socket.recv::<Msg>().await.map_err(Error::Io)?;
            let entry = self.sessions.entry(addr.clone()).or_default();
            match (core::mem::take(entry), msg) {
                (NoiseSession::Initiating, Msg::Hello) => {
                    let mut hs = Box::new(builder(true, self.private_key));
                    let len = hs.write_message(&[], &mut hs_buf).map_err(Error::Noise)?;
                    send_first = Some((Msg::XX1(hs_buf[..len].to_vec()), addr));
                    let mut xx1 = [0u8; 32];
                    xx1.copy_from_slice(&hs_buf[..32]);
                    *entry = NoiseSession::XX1Sent(xx1, hs);
                }
                (NoiseSession::XX1Sent(xx1, _), Msg::XX1(vec)) => {
                    if vec.as_slice() > xx1.as_slice() {
                        let mut hs = Box::new(builder(false, self.private_key));
                        hs.read_message(vec.as_slice(), &mut hs_buf)
                            .map_err(Error::Noise)?;
                        let len = hs.write_message(&[], &mut hs_buf).map_err(Error::Noise)?;
                        send_first = Some((Msg::XX2(hs_buf[..len].to_vec()), addr));
                        *entry = NoiseSession::XX2Sent(hs);
                    }
                }
                (NoiseSession::XX1Sent(_, mut hs), Msg::XX2(vec)) => {
                    // <- 2
                    hs.read_message(&vec, &mut hs_buf).map_err(Error::Noise)?;
                    // -> 3
                    let len = hs.write_message(&[], &mut hs_buf).map_err(Error::Noise)?;
                    send_first = Some((Msg::XX3(hs_buf[..len].to_vec()), addr));
                    let transport = hs.into_transport_mode().map_err(Error::Noise)?;
                    *entry = NoiseSession::Transport(transport);
                }
                (NoiseSession::XX2Sent(mut hs), Msg::XX3(msg)) => {
                    hs.read_message(&msg, &mut hs_buf).map_err(Error::Noise)?;
                    let transport = hs.into_transport_mode().map_err(Error::Noise)?;
                    *entry = NoiseSession::Transport(transport);
                }
                (NoiseSession::Transport(mut t), Msg::Payload(nonce, msg)) => {
                    if nonce >= t.receiving_nonce() {
                        t.set_receiving_nonce(nonce);
                    }
                    let len = t.read_message(&msg, &mut hs_buf).map_err(Error::Noise)?;
                    *entry = NoiseSession::Transport(t);
                    let ret = postcard::from_bytes(&hs_buf[..len]).map_err(Error::Serde)?;
                    return Ok((ret, entry.get_remote_static().unwrap()));
                }

                (_, Msg::XX1(msg)) => {
                    let mut hs = Box::new(builder(false, self.private_key));
                    hs.read_message(msg.as_slice(), &mut hs_buf)
                        .map_err(Error::Noise)?;
                    let len = hs.write_message(&[], &mut hs_buf).map_err(Error::Noise)?;
                    send_first = Some((Msg::XX2(hs_buf[..len].to_vec()), addr));
                    *entry = NoiseSession::XX2Sent(hs);
                }
                (_, Msg::Payload(_, _)) => {
                    let mut hs = Box::new(builder(true, self.private_key));
                    let len = hs.write_message(&[], &mut hs_buf).map_err(Error::Noise)?;
                    send_first = Some((Msg::XX1(hs_buf[..len].to_vec()), addr));
                    let mut xx1 = [0u8; 32];
                    xx1.copy_from_slice(&hs_buf[..32]);
                    *entry = NoiseSession::XX1Sent(xx1, hs);
                }
                _ => {}
            }
        }
    }
}
