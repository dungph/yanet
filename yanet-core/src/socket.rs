use futures_micro::Future;

pub trait Socket: Sized {
    type Addr;
    type Error;

    async fn broadcast(&self, buf: &[u8]) -> Result<(), Self::Error>;
    async fn send(&self, buf: &[u8], addr: Self::Addr) -> Result<usize, Self::Error>;
    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, Self::Addr), Self::Error>;
    async fn or<S: Socket>(self, other: S) -> Or<Self, S> {
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

    async fn send(&self, buf: &[u8], addr: Self::Addr) -> Result<usize, Self::Error> {
        match addr {
            Either::This(addr) => Ok(self.this.send(buf, addr).await.map_err(Either::This)?),
            Either::Other(addr) => Ok(self.other.send(buf, addr).await.map_err(Either::Other)?),
        }
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, Self::Addr), Self::Error> {
        loop {
            {
                let future = self.this.recv(buf);
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
                let future = self.other.recv(buf);
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

    async fn broadcast(&self, buf: &[u8]) -> Result<(), Self::Error> {
        self.this.broadcast(buf).await.map_err(Either::This)?;
        self.other.broadcast(buf).await.map_err(Either::Other)?;
        Ok(())
    }
}
