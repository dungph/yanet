use core::fmt::Debug;
use core::fmt::Display;

use serde::{de::DeserializeOwned, Serialize};

pub trait Authenticated {
    fn peer_id(&self) -> [u8; 32];
}
pub trait Channel: Sized {
    type Error: Display + Debug + Send + Sync + 'static;

    fn is_initiator(&self) -> bool;
    async fn recv(&self) -> Result<Vec<u8>, Self::Error>;
    async fn send(&self, buf: &[u8]) -> Result<(), Self::Error>;

    async fn recv_postcard<T>(&self) -> Result<T, anyhow::Error>
    where
        T: DeserializeOwned,
    {
        let vec = self.recv().await.map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(postcard::from_bytes(&vec)?)
    }
    async fn send_postcard(&self, value: &impl Serialize) -> Result<(), anyhow::Error> {
        let buf = postcard::to_allocvec(value)?;
        self.send(&buf)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(())
    }
}

mod test {
    struct AnyhowChannel;
    impl super::Channel for AnyhowChannel {
        type Error = anyhow::Error;

        fn is_initiator(&self) -> bool {
            true
        }

        async fn recv(&self) -> Result<Vec<u8>, Self::Error> {
            todo!()
        }

        async fn send(&self, buf: &[u8]) -> Result<(), Self::Error> {
            todo!()
        }
    }
}
