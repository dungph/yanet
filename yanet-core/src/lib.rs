#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![no_std]

pub mod socket;
pub use socket::Socket;

pub trait SocketUpgrade<S>: ServiceName {
    type Output;
    type Error;
    async fn upgrade(&self, socket: S) -> Result<Self::Output, Self::Error>;
}

pub trait ServiceName {
    type Name;
    fn name(&self) -> Self::Name;
}
