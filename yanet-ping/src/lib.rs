#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
use std::time::Duration;

use yanet_core::{Service, ServiceName, Socket};

pub struct Pinger {
    dur: Duration,
}

impl Pinger {
    pub fn new(dur: Duration) -> Self {
        Self { dur }
    }
}
impl ServiceName for Pinger {
    type Name = &'static str;

    fn name(&self) -> Self::Name {
        "pinger"
    }
}
impl<S> Service<S> for Pinger
where
    S: Socket + Clone,
    S::Addr: std::fmt::Debug,
{
    type Output = ();

    type Error = S::Error;

    async fn upgrade(&self, mut socket: S) -> Result<Self::Output, Self::Error> {
        let mut socket1 = socket.clone();
        let task1 = async {
            loop {
                futures_timer::Delay::new(self.dur).await;
                socket1.broadcast(&"hello").await?;
            }
        };
        let task2 = async {
            loop {
                let (s, a) = socket.recv::<String>().await?;
                println!("received {} from {:?}", s, a);
            }
        };
        futures_micro::or!(task1, task2).await
    }
}
