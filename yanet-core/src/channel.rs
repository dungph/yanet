use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};

use crate::Either;

pub trait Channel: Sized {
    fn is_initiator(&self) -> bool;
    async fn recv(&self) -> Result<Vec<u8>>;
    async fn send(&self, buf: &[u8]) -> Result<()>;

    async fn recv_postcard<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let vec = self.recv().await?;
        Ok(postcard::from_bytes(&vec)?)
    }
    async fn send_postcard(&self, value: &impl Serialize) -> Result<()> {
        let buf = postcard::to_allocvec(value)?;
        self.send(&buf).await?;
        Ok(())
    }
}

impl<A, B> Channel for Either<A, B>
where
    A: Channel,
    B: Channel,
{
    fn is_initiator(&self) -> bool {
        match self {
            Either::A(a) => a.is_initiator(),
            Either::B(b) => b.is_initiator(),
        }
    }

    async fn recv(&self) -> Result<Vec<u8>> {
        match self {
            Either::A(a) => a.recv().await,
            Either::B(b) => b.recv().await,
        }
    }

    async fn send(&self, buf: &[u8]) -> Result<()> {
        match self {
            Either::A(a) => a.send(buf).await,
            Either::B(b) => b.send(buf).await,
        }
    }
}
