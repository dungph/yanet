#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
use std::time::{Duration, Instant};

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
    S: Socket,
    S::Addr: std::fmt::Debug,
{
    type Output = ();

    type Error = S::Error;

    async fn upgrade(&self, mut socket: S) -> Result<Self::Output, Self::Error> {
        let mut start = Instant::now();
        loop {
            if start.elapsed() > self.dur {
                socket.broadcast(&"hello").await?;
                start = Instant::now();
            }
            let sleep = async {
                futures_timer::Delay::new(self.dur - start.elapsed()).await;
                None
            };
            let recv = async { Some(socket.recv::<String>().await) };
            if let Some((s, a)) = futures_micro::or!(sleep, recv).await.transpose()? {
                println!("received {} from {:?}", s, a);
            }
        }
    }
}
