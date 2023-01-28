//use yanet_core::{Authenticated, Channel, Service, ServiceName};
//use yanet_messaging::MessageService;
//
//pub struct PairService {
//    messaging: MessageService,
//}
//
//impl PairService {
//    pub fn new() -> Self {
//        Self {
//            messaging: MessageService::new(),
//        }
//    }
//    pub async fn broadcast(&self, data: &impl Serialize) {}
//}
//impl ServiceName for PairService {
//    type Name = &'static str;
//
//    fn name(&self) -> Self::Name {
//        "thing-pair"
//    }
//}
//
//impl<C: Authenticated + Channel> Service<C> for PairService {
//    type Output = <MessageService as Service<C>>::Output;
//
//    async fn upgrade(&self, channel: C) -> anyhow::Result<Self::Output> {
//        self.messaging.upgrade(channel).await
//    }
//}
