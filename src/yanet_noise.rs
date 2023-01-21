use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use snow::TransportState;

use crate::{
    utils::FutureTimeout,
    yanet_core::{
        channel::{Authenticated, Channel},
        upgrade::{Named, Upgrade},
    },
};

#[derive(Serialize, Deserialize)]
enum Message<P: AsRef<[u8]> + Sized> {
    BeginHandshake,
    Payload(P),
}
pub struct NoiseService {
    private_key: [u8; 32],
    shared_buf: Rc<RefCell<[u8; 10240]>>,
}

impl NoiseService {
    pub fn new(get_key: impl Fn() -> [u8; 32]) -> Self {
        Self {
            private_key: get_key(),
            shared_buf: Rc::new(RefCell::new([0u8; 10240])),
        }
    }
}

impl Named for NoiseService {
    fn name(&self) -> &str {
        "noise"
    }
}
impl<C> Upgrade<C> for NoiseService
where
    C: Channel,
{
    type Output = NoiseChannel<C>;
    type Error = anyhow::Error;
    async fn upgrade(&self, channel: C) -> Result<Self::Output, Self::Error> {
        Ok(NoiseChannel::new_handshake(self.private_key, channel, self.shared_buf.clone()).await?)
    }
}

pub struct NoiseChannel<T> {
    private_key: [u8; 32],
    transport: RefCell<TransportState>,
    channel: T,
    shared_buf: Rc<RefCell<[u8; 10240]>>,
}

impl<T> NoiseChannel<T>
where
    T: Channel,
{
    pub async fn new_handshake(
        private_key: [u8; 32],
        channel: T,
        shared_buf: Rc<RefCell<[u8; 10240]>>,
    ) -> anyhow::Result<Self> {
        let transport = if channel.is_initiator() {
            let message: Message<&[u8]> = Message::BeginHandshake;
            channel
                .send_postcard(&message)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            Self::handshake(&private_key, &channel, true).await?
        } else {
            let msg: Option<Message<Vec<u8>>> = channel
                .recv_postcard()
                .timeout(Duration::from_millis(1000))
                .await
                .transpose()?;

            let is_initiator = match msg {
                Some(Message::BeginHandshake) => false,
                Some(Message::Payload(_)) => true,
                None => true,
            };

            if is_initiator {
                let msg: Message<&[u8]> = Message::BeginHandshake;
                channel.send_postcard(&msg).await?;
            }
            Self::handshake(&private_key, &channel, is_initiator).await?
        };
        Ok(Self {
            private_key,
            transport: RefCell::new(transport),
            channel,
            shared_buf,
        })
    }

    async fn handshake<C>(
        private_key: &[u8; 32],
        channel: &C,
        is_initiator: bool,
    ) -> anyhow::Result<TransportState>
    where
        C: Channel,
    {
        let builder = snow::Builder::new("Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap())
            .local_private_key(private_key);

        loop {
            let mut handshake = if is_initiator {
                builder.build_initiator().unwrap()
            } else {
                builder.build_responder().unwrap()
            };

            let mut buf = [0u8; 128];
            while !handshake.is_handshake_finished() {
                if handshake.is_my_turn() {
                    let len = handshake.write_message(&[], &mut buf)?;
                    channel
                        .send_postcard(&Message::Payload(&buf[..len]))
                        .await?;
                } else {
                    let msg: Message<Vec<u8>> = channel.recv_postcard().await?;
                    match msg {
                        Message::Payload(recv) => {
                            handshake.read_message(&recv, &mut buf)?;
                        }
                        Message::BeginHandshake => continue,
                    }
                }
            }
            break Ok(handshake.into_transport_mode().unwrap());
        }
    }
}
impl<T> Authenticated for NoiseChannel<T> {
    fn peer_id(&self) -> [u8; 32] {
        self.transport
            .borrow()
            .get_remote_static()
            .unwrap()
            .try_into()
            .expect("Only accept x25519")
    }
}

impl<T> Channel for NoiseChannel<T>
where
    T: Channel,
    T::Error: Into<anyhow::Error>,
{
    type Error = anyhow::Error;

    fn is_initiator(&self) -> bool {
        !self.transport.borrow().is_initiator()
    }
    async fn recv(&self) -> Result<Vec<u8>, Self::Error> {
        loop {
            let message: Message<Vec<u8>> = self
                .channel
                .recv_postcard()
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            match message {
                Message::Payload(recv) => {
                    let mut buf = self.shared_buf.borrow_mut();
                    let len = self.transport.borrow_mut().read_message(&recv, &mut *buf)?;
                    break Ok(buf[..len].to_owned());
                }
                Message::BeginHandshake => {
                    *self.transport.borrow_mut() =
                        Self::handshake(&self.private_key, &self.channel, false).await?;
                }
            }
        }
    }
    async fn send(&self, payload: &[u8]) -> Result<(), Self::Error> {
        let mut buf = self.shared_buf.borrow_mut();
        let len = self
            .transport
            .borrow_mut()
            .write_message(payload, &mut *buf)?;

        self.channel
            .send_postcard(&Message::Payload(&buf[..len]))
            .await?;
        Ok(())
    }
}
