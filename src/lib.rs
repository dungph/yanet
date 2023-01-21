#![feature(async_fn_in_trait)]
#![feature(error_in_core)]

pub mod yanet_core;
pub mod yanet_multiplex;
pub mod yanet_noise;
pub mod yanet_relay;

pub use yanet_core::channel::{Authenticated, Channel};
pub use yanet_core::transport::Transport;
pub use yanet_core::upgrade::{Named, Upgrade};
