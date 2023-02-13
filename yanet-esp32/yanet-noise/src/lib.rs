#![feature(async_fn_in_trait)]

use std::cell::RefCell;

use anyhow::Result;
use async_channel::{unbounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use snow::{HandshakeState, TransportState};
use yanet_core::{authenticate::PeerId, Authenticated, Channel, Service, ServiceName};

#[derive(Serialize, Deserialize, Debug)]
enum Msg {
    IX1(Vec<u8>),
    IX2(Vec<u8>),
    Payload(u64, Vec<u8>),
}

pub struct NoiseService {
    private_key: [u8; 32],
    next_peer: (Sender<PeerId>, Receiver<PeerId>),
}

impl NoiseService {
    pub fn new(get_key: impl FnOnce() -> [u8; 32]) -> Self {
        Self {
            private_key: get_key(),
            next_peer: unbounded(),
        }
    }

    pub async fn next_peer(&self) -> PeerId {
        self.next_peer.1.recv().await.unwrap()
    }
}
fn builder(init: bool, pkey: [u8; 32]) -> HandshakeState {
    let builder = snow::Builder::new("Noise_IX_25519_ChaChaPoly_BLAKE2s".parse().unwrap())
        .local_private_key(&pkey);
    if init {
        builder.build_initiator().unwrap()
    } else {
        builder.build_responder().unwrap()
    }
}

async fn respond(channel: &impl Channel, pkey: [u8; 32], ix1: &[u8]) -> Result<TransportState> {
    let mut buf = [0u8; 128];
    let mut hs = builder(false, pkey);
    hs.read_message(ix1, &mut buf)?;
    let len = hs.write_message(&[], &mut buf)?;
    let ix2 = Msg::IX2(buf[..len].to_vec());
    dbg!(channel.send_postcard(&ix2).await)?;
    Ok(hs.into_transport_mode().unwrap())
}
async fn init(channel: &impl Channel, pkey: [u8; 32]) -> Result<TransportState> {
    let mut buf = [0u8; 128];
    let mut hs = builder(true, pkey);
    let len = dbg!(hs.write_message(&[], &mut buf))?;
    let ix1_data = Msg::IX1(buf[..len].to_vec());
    dbg!(channel.send_postcard(&ix1_data).await)?;
    loop {
        let data = dbg!(channel.recv_postcard::<Msg>().await)?;
        match data {
            Msg::IX1(data) => {
                if data > buf[..len].to_vec() {
                    break Ok(respond(channel, pkey, &data).await?);
                }
            }
            Msg::IX2(data) => {
                hs.read_message(&data, &mut buf)?;
                break Ok(hs.into_transport_mode().unwrap());
            }
            _ => (),
        }
    }
}
impl ServiceName for NoiseService {
    type Name = &'static str;
    fn name(&self) -> Self::Name {
        "noise"
    }
}

impl<C: Channel + 'static> Service<C> for NoiseService {
    type Output = NoiseChannel;
    async fn upgrade(&self, channel: C) -> Result<Self::Output> {
        println!("begin noise upgrade");
        let (tx, rx2) = unbounded::<Vec<u8>>();
        let (tx2, rx) = unbounded::<Vec<u8>>();
        let mut transport = RefCell::new(dbg!(init(&channel, self.private_key).await)?);
        let peer_id: PeerId = transport
            .borrow()
            .get_remote_static()
            .unwrap()
            .try_into()
            .unwrap();
        let private_key = self.private_key;
        let task = async move {
            let task1 = async {
                let mut buf = [0u8; 512];
                while let Ok(msg) = channel.recv_postcard::<Msg>().await {
                    match msg {
                        Msg::Payload(nonce, data) => {
                            transport.borrow_mut().set_receiving_nonce(nonce);
                            let len = transport.borrow_mut().read_message(&data, &mut buf)?;
                            tx2.send(buf[..len].to_vec()).await?;
                        }
                        Msg::IX1(data) => {
                            *transport.borrow_mut() = respond(&channel, private_key, &data).await?;
                        }
                        Msg::IX2(_) => {
                            *transport.borrow_mut() = init(&channel, private_key).await?;
                        }
                    }
                }
                Ok(()) as anyhow::Result<()>
            };
            let task2 = async {
                let mut buf = [0u8; 512];
                while let Ok(msg) = rx2.recv().await {
                    let nonce = transport.borrow().sending_nonce();
                    let len = transport.borrow_mut().write_message(&msg, &mut buf)?;
                    channel
                        .send_postcard(&Msg::Payload(nonce, buf[..len].to_vec()))
                        .await?;
                }
                Ok(())
            };
            futures_lite::future::or(task1, task2).await.ok();
        };
        local_ex::spawn(task).detach();
        self.next_peer.0.send(peer_id.clone()).await.ok();
        Ok(NoiseChannel {
            peer_id: *peer_id,
            tx,
            rx,
        })
    }
}

pub struct NoiseChannel {
    peer_id: [u8; 32],
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
}

impl Authenticated for NoiseChannel {
    fn peer_id(&self) -> PeerId {
        self.peer_id.into()
    }
}

impl Channel for NoiseChannel {
    fn is_initiator(&self) -> bool {
        true
    }
    async fn recv(&self) -> Result<Vec<u8>> {
        Ok(self.rx.recv().await?)
    }
    async fn send(&self, payload: &[u8]) -> Result<()> {
        self.tx.send(payload.to_vec()).await?;
        Ok(())
    }
}
