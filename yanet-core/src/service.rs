pub trait Service<C>: ServiceName + Sized {
    type Output;

    async fn upgrade(&self, channel: C) -> anyhow::Result<Self::Output>;
}

pub trait ServiceName {
    type Name: AsRef<[u8]>;
    fn name(&self) -> Self::Name;
}

impl<T> ServiceName for &T
where
    T: ServiceName,
{
    type Name = T::Name;
    fn name(&self) -> T::Name {
        (*self).name()
    }
}

impl<T, C> Service<C> for &T
where
    T: Service<C>,
{
    type Output = T::Output;

    async fn upgrade(&self, a: C) -> anyhow::Result<T::Output> {
        Ok((*self).upgrade(a).await?)
    }
}
