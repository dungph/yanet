use futures_micro::Future;
use serde::{de::DeserializeOwned, Serialize};

pub trait Socket: Sized {
    type Addr;
    type Error;
    async fn broadcast<D>(&self, data: &D) -> Result<(), Self::Error>
    where
        D: Serialize;

    async fn send<D>(&self, data: &D, addr: Self::Addr) -> Result<(), Self::Error>
    where
        D: Serialize + ?Sized;

    async fn recv<D>(&self) -> Result<(D, Self::Addr), Self::Error>
    where
        D: DeserializeOwned;

    fn or<S: Socket>(self, other: S) -> Or<Self, S> {
        Or { this: self, other }
    }
}

pub struct Or<T, O> {
    this: T,
    other: O,
}

#[derive(PartialEq, PartialOrd, Ord, Eq)]
pub enum Either<T, O> {
    This(T),
    Other(O),
}

impl<T: Socket, O: Socket> Socket for Or<T, O> {
    type Addr = Either<T::Addr, O::Addr>;

    type Error = Either<T::Error, O::Error>;

    async fn broadcast<D>(&self, data: &D) -> Result<(), Self::Error>
    where
        D: Serialize,
    {
        self.this.broadcast(data).await.map_err(Either::This)?;
        self.other.broadcast(data).await.map_err(Either::Other)?;
        Ok(())
    }

    async fn send<D>(&self, data: &D, addr: Self::Addr) -> Result<(), Self::Error>
    where
        D: Serialize + ?Sized,
    {
        match addr {
            Either::This(addr) => Ok(self.this.send(data, addr).await.map_err(Either::This)?),
            Either::Other(addr) => Ok(self.other.send(data, addr).await.map_err(Either::Other)?),
        }
    }

    async fn recv<D>(&self) -> Result<(D, Self::Addr), Self::Error>
    where
        D: DeserializeOwned,
    {
        loop {
            {
                let future = self.this.recv();
                futures_micro::pin!(future);
                let waker = futures_micro::waker().await;
                let mut context = futures_micro::Context::from_waker(&waker);
                match future.poll(&mut context) {
                    futures_micro::Poll::Ready(Ok((s, a))) => return Ok((s, Either::This(a))),
                    futures_micro::Poll::Ready(Err(e)) => return Err(Either::This(e)),
                    futures_micro::Poll::Pending => (),
                }
            }
            {
                let future = self.other.recv();
                futures_micro::pin!(future);
                let waker = futures_micro::waker().await;
                let mut context = futures_micro::Context::from_waker(&waker);
                match future.poll(&mut context) {
                    futures_micro::Poll::Ready(Ok((s, a))) => return Ok((s, Either::Other(a))),
                    futures_micro::Poll::Ready(Err(e)) => return Err(Either::Other(e)),
                    futures_micro::Poll::Pending => (),
                }
            }
        }
    }
}
