#![feature(async_fn_in_trait)]

use futures_lite::Future;
use std::time::Duration;

pub trait FutureTimeout: Future + Sized {
    async fn timeout(self, dur: Duration) -> Option<Self::Output> {
        let task1 = async { Some(self.await) };
        let task2 = async {
            futures_timer::Delay::new(dur).await;
            None
        };
        futures_lite::future::or(task1, task2).await
    }
    async fn timeout_millis(self, millis: u64) -> Option<Self::Output> {
        self.timeout(Duration::from_millis(millis)).await
    }
    async fn timeout_secs(self, secs: u64) -> Option<Self::Output> {
        self.timeout(Duration::from_secs(secs)).await
    }
}

impl<T: Future + Sized> FutureTimeout for T {}
