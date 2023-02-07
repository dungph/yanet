#![feature(async_fn_in_trait)]

use anyhow::Result;
use async_channel::{unbounded, Receiver, Sender};
use log::{debug, error, info, trace, warn};
use snow::TransportState;
use std::cell::RefCell;
use yanet_core::{authenticate::PeerId, Authenticated, Channel, Service, ServiceName};

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

impl ServiceName for NoiseService {
    type Name = &'static str;
    fn name(&self) -> Self::Name {
        "noise"
    }
}

impl<C: Channel> Service<C> for NoiseService {
    type Output = NoiseChannel<C>;
    async fn upgrade(&self, channel: C) -> Result<Self::Output> {
        let builder = snow::Builder::new("Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap())
            .local_private_key(&self.private_key);

        let mut handshake = if channel.is_initiator() {
            trace!("build initiator");
            builder.build_initiator().unwrap()
        } else {
            trace!("build responder");
            builder.build_responder().unwrap()
        };
        let transport = {
            loop {
                let mut buf = [0u8; 128];
                while !handshake.is_handshake_finished() {
                    if handshake.is_my_turn() {
                        let len = handshake.write_message(&[], &mut buf)?;
                        channel
                            .send(&buf[..len])
                            .await
                            .map_err(|e| anyhow::anyhow!("{}", e))?;
                    } else {
                        let msg = channel.recv().await.map_err(|e| anyhow::anyhow!("{}", e))?;
                        handshake.read_message(&msg, &mut buf)?;
                    }
                }
                break handshake.into_transport_mode().unwrap();
            }
        };

        let pubkey: [u8; 32] = transport.get_remote_static().unwrap().try_into().unwrap();
        self.next_peer.0.send(pubkey.into()).await?;
        Ok(NoiseChannel {
            transport: RefCell::new(transport),
            channel,
        })
    }
}

pub struct NoiseChannel<T> {
    transport: RefCell<TransportState>,
    channel: T,
}

impl<T> Authenticated for NoiseChannel<T> {
    fn peer_id(&self) -> PeerId {
        let key: [u8; 32] = self
            .transport
            .borrow()
            .get_remote_static()
            .unwrap()
            .try_into()
            .expect("Only accept x25519");
        key.into()
    }
}

impl<T> Channel for NoiseChannel<T>
where
    T: Channel,
{
    fn is_initiator(&self) -> bool {
        self.transport.borrow().is_initiator()
    }
    async fn recv(&self) -> Result<Vec<u8>> {
        let message = self
            .channel
            .recv()
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let mut buf = [0u8; 1024];
        let len = self
            .transport
            .borrow_mut()
            .read_message(&message, &mut buf)?;
        Ok(buf[..len].to_owned())
    }
    async fn send(&self, payload: &[u8]) -> Result<()> {
        let mut buf = [0u8; 1024];
        let len = self
            .transport
            .borrow_mut()
            .write_message(payload, &mut buf)?;

        self.channel
            .send(&buf[..len])
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(())
    }
}
