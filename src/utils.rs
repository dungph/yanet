use std::time::Duration;

use futures_lite::Future;

pub(crate) trait FutureTimeout: Future + Sized {
    async fn timeout(self, dur: Duration) -> Option<Self::Output> {
        let task1 = async { Some(self.await) };
        let task2 = async {
            futures_timer::Delay::new(dur).await;
            None
        };
        futures_lite::future::or(task1, task2).await
    }
}

impl<T: Future + Sized> FutureTimeout for T {}
