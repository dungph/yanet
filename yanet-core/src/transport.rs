use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use crate::{Either, Or};

use super::{channel::Channel, service::Service};

pub trait Transport: Sized {
    type Channel;
    async fn get(&self) -> anyhow::Result<Option<Self::Channel>>;

    fn or<T>(self, other: T) -> OrTransport<Self, T> {
        OrTransport {
            a_done: AtomicBool::new(false),
            b_done: AtomicBool::new(false),
            a: self,
            b: other,
        }
    }

    fn then<U>(self, upgrader: U) -> Then<Self, U> {
        Then {
            channel: self,
            service: upgrader,
        }
    }
    async fn handle<U>(self, upgrader: U) -> anyhow::Result<()>
    where
        U: Service<Self::Channel>,
        U::Output: std::fmt::Debug,
    {
        let ex = async_executor::LocalExecutor::new();
        let task = async {
            loop {
                let ch = self.get().await?;
                match ch {
                    Some(ch) => {
                        ex.spawn(upgrader.upgrade(ch)).detach();
                    }
                    None => break Ok(()),
                }
            }
        };
        let task2 = async {
            loop {
                ex.tick().await;
            }
        };
        futures_lite::future::or(task, task2).await
    }
}

pub struct OrTransport<A, B> {
    a_done: AtomicBool,
    b_done: AtomicBool,
    a: A,
    b: B,
}

impl<A: Transport, B: Transport> Transport for OrTransport<A, B> {
    type Channel = Either<A::Channel, B::Channel>;

    async fn get(&self) -> anyhow::Result<Option<Self::Channel>> {
        if self.a_done.load(Relaxed) && self.b_done.load(Relaxed) {
            Ok(None)
        } else if self.a_done.load(Relaxed) {
            if let Some(b) = self.b.get().await? {
                Ok(Some(Either::B(b)))
            } else {
                self.b_done.store(true, Relaxed);
                Ok(None)
            }
        } else if self.b_done.load(Relaxed) {
            if let Some(a) = self.a.get().await? {
                Ok(Some(Either::A(a)))
            } else {
                self.a_done.store(true, Relaxed);
                Ok(None)
            }
        } else {
            let task1 = async { Ok(Either::A(self.a.get().await?)) as anyhow::Result<_> };
            let task2 = async { Ok(Either::B(self.b.get().await?)) };
            let ret = futures_lite::future::or(task1, task2).await?;
            match ret {
                Either::A(Some(a)) => Ok(Some(Either::A(a))),
                Either::B(Some(b)) => Ok(Some(Either::B(b))),
                Either::A(None) => {
                    self.a_done.store(true, Relaxed);
                    Ok(None)
                }
                Either::B(None) => {
                    self.b_done.store(true, Relaxed);
                    Ok(None)
                }
            }
        }
    }
}

pub struct Then<C, S> {
    pub(crate) channel: C,
    pub(crate) service: S,
}

impl<A, B> Transport for Then<A, B>
where
    A: Transport,
    A::Channel: Channel,
    B: Service<A::Channel>,
{
    type Channel = B::Output;

    async fn get(&self) -> anyhow::Result<Option<<Then<A, B> as Transport>::Channel>> {
        if let Some(channel) = self.channel.get().await? {
            Ok(Some(self.service.upgrade(channel).await?))
        } else {
            Ok(None)
        }
    }
}
impl<T> Transport for &T
where
    T: Transport,
{
    type Channel = T::Channel;

    async fn get(&self) -> anyhow::Result<Option<<&T as Transport>::Channel>> {
        (*self).get().await
    }
}
