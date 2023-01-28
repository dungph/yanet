#![feature(async_fn_in_trait)]

pub mod authenticate;
pub mod channel;
pub mod service;
pub mod transport;

pub use authenticate::Authenticated;
pub use authenticate::PeerId;
pub use channel::Channel;
pub use service::{Service, ServiceName};
pub use transport::Transport;

pub struct Or<A, B> {
    a: A,
    b: B,
}

pub enum Either<A, B> {
    A(A),
    B(B),
}
