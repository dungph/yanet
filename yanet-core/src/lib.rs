#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![no_std]

pub mod socket;
pub use socket::Socket;
pub mod service;
pub use service::{Service, ServiceName};
