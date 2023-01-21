use core::fmt::{Debug, Display};

pub trait Upgrade<C>: Named + Sized {
    type Output;
    type Error: Display + Debug + Send + Sync + 'static;

    async fn upgrade(&self, channel: C) -> Result<Self::Output, Self::Error>;
}

pub trait Named {
    fn name(&self) -> &str;
}

impl<T> Named for &T
where
    T: Named,
{
    fn name(&self) -> &str {
        (*self).name()
    }
}

impl<T, A> Upgrade<A> for &T
where
    T: Upgrade<A>,
{
    type Output = T::Output;
    type Error = T::Error;

    async fn upgrade(&self, a: A) -> Result<<&T as Upgrade<A>>::Output, <&T as Upgrade<A>>::Error> {
        Ok((*self).upgrade(a).await?)
    }
}
