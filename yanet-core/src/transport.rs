use crate::{Either, Or};

use super::{channel::Channel, service::Service};

pub trait Transport: Sized {
    type Channel;
    async fn get(&self) -> Self::Channel;

    fn or<T>(self, other: T) -> Or<Self, T> {
        Or { a: self, b: other }
    }

    fn then<U>(self, upgrader: U) -> Then<Self, U> {
        Then {
            channel: self,
            service: upgrader,
        }
    }
    async fn handle<U>(self, upgrader: U)
    where
        U: Service<Self::Channel>,
        U::Output: std::fmt::Debug,
    {
        let ex = async_executor::LocalExecutor::new();
        let task = async {
            loop {
                let ch = self.get().await;
                ex.spawn(upgrader.upgrade(ch)).detach();
            }
        };
        let task2 = async {
            loop {
                ex.tick().await;
            }
        };
        futures_lite::future::or(task, task2).await;
    }
}

impl<A: Transport, B: Transport> Transport for Or<A, B> {
    type Channel = Either<A::Channel, B::Channel>;

    async fn get(&self) -> Self::Channel {
        let task1 = async { Either::A(self.a.get().await) };
        let task2 = async { Either::B(self.b.get().await) };
        futures_lite::future::or(task1, task2).await
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

    async fn get(&self) -> <Then<A, B> as Transport>::Channel {
        loop {
            let channel = self.channel.get().await;
            if let Ok(out) = self.service.upgrade(channel).await {
                break out;
            }
        }
    }
}
impl<T> Transport for &T
where
    T: Transport,
{
    type Channel = T::Channel;

    async fn get(&self) -> <&T as Transport>::Channel {
        (*self).get().await
    }
}
