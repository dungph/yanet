pub trait Service<S>: ServiceName {
    type Output;
    type Error;
    async fn upgrade(&self, socket: S) -> Result<Self::Output, Self::Error>;
}

pub trait ServiceName {
    type Name;
    fn name(&self) -> Self::Name;
}
