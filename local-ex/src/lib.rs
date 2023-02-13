use std::{
    future::Future,
    task::{Context, Poll},
};

use async_executor::{LocalExecutor, Task};

thread_local!(static EX: LocalExecutor<'static> = LocalExecutor::new());

pub fn spawn<T: 'static>(fut: impl Future<Output = T> + 'static) -> Task<T> {
    EX.with(|e| e.spawn(fut))
}

pub fn run<F: Future>(task: F) -> F::Output {
    EX.with(|ex| {
        futures_lite::pin!(task);

        let this = std::thread::current();
        let waker = waker_fn::waker_fn(move || {
            this.unpark();
        });
        let mut cx = Context::from_waker(&waker);

        loop {
            if let Poll::Ready(r) = task.as_mut().poll(&mut cx) {
                return r;
            }
            while ex.try_tick() {}

            let fut = ex.tick();
            futures_lite::pin!(fut);

            match fut.poll(&mut cx) {
                Poll::Ready(_) => (),
                Poll::Pending => std::thread::park(),
            }
        }
    })
}
