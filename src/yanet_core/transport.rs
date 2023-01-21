use super::{channel::Channel, helper::Then, upgrade::Upgrade};

pub trait Transport: Sized {
    type Item;
    async fn get(&self) -> Self::Item;
    async fn consume_each(self) {
        loop {
            self.get().await;
        }
    }
    fn then<U>(self, upgrader: U) -> Then<Self, U> {
        Then {
            channel: self,
            service: upgrader,
        }
    }
}

impl<A, B> Transport for Then<A, B>
where
    A: Transport,
    A::Item: Channel,
    B: Upgrade<A::Item>,
{
    type Item = B::Output;

    async fn get(&self) -> <Then<A, B> as Transport>::Item {
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
    type Item = T::Item;

    async fn get(&self) -> <&T as Transport>::Item {
        (*self).get().await
    }
}
