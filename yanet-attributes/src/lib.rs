#![feature(async_fn_in_trait)]

use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};

use async_channel::{bounded, unbounded, Receiver, Sender};
use event_listener::Event;
use futures_lite::future::yield_now;
use serde::{Deserialize, Serialize};
use yanet_core::{authenticate::PeerId, Authenticated, Channel, Service, ServiceName};

#[derive(Serialize, Deserialize, Debug)]
enum Message {
    Sync,
    Value(String, Value),
    Set(String, Value),
}

#[derive(Serialize, Deserialize)]
pub struct Attribute {
    value: Value,
    actions: Vec<Action>,

    #[serde(skip)]
    listener: Rc<Event>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum Action {
    DirectMap(String),
    Toggle(String),
    Increase(String, f32),
    Decrease(String, f32),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    Blob(Vec<u8>),
    List(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Into<serde_json::Value> for Value {
    fn into(self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => b.into(),
            Value::Number(n) => n.into(),
            Value::Text(t) => t.into(),
            Value::Blob(b) => base64::encode(b).into(),
            Value::List(list) => list
                .into_iter()
                .map(|e| e.into())
                .collect::<Vec<serde_json::Value>>()
                .into(),
            Value::Object(map) => map
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect::<serde_json::Map<_, serde_json::Value>>()
                .into(),
        }
    }
}
impl From<serde_json::Value> for Value {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(n) => Self::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => Self::Text(s),
            serde_json::Value::Array(a) => {
                Self::List(a.into_iter().map(|v| Value::from(v)).collect())
            }
            serde_json::Value::Object(m) => {
                Self::Object(m.into_iter().map(|(k, v)| (k, Value::from(v))).collect())
            }
        }
    }
}
pub struct AttributesService {
    peer_id: PeerId,
    peers: RefCell<BTreeMap<PeerId, Sender<Message>>>,
    attributes: RefCell<BTreeMap<PeerId, BTreeMap<String, Attribute>>>,
    sync_new_peer: AtomicBool,
    recv_any: RefCell<Option<Sender<(PeerId, String, Value)>>>,
}

impl AttributesService {
    pub fn new(peer: PeerId) -> Self {
        Self {
            peer_id: peer,
            attributes: RefCell::new(BTreeMap::new()),
            peers: RefCell::new(BTreeMap::new()),
            sync_new_peer: AtomicBool::new(false),
            recv_any: RefCell::new(None),
        }
    }

    pub fn set_recv_any(&self) -> Receiver<(PeerId, String, Value)> {
        let (tx, rx) = unbounded();
        self.recv_any.borrow_mut().replace(tx);
        rx
    }
    pub fn sync_new_peer(&self, syn: bool) {
        self.sync_new_peer.store(syn, Relaxed)
    }
    pub fn add_action(&self, peer: PeerId, name: &str, action: Action) {
        self.attributes
            .borrow_mut()
            .entry(peer)
            .or_insert_with(|| BTreeMap::new())
            .entry(name.to_owned())
            .and_modify(|at| at.actions.retain(|a| a.eq(&action)))
            .or_insert_with(|| Attribute {
                value: Value::Null,
                listener: Rc::new(Event::new()),
                actions: Vec::new(),
            })
            .actions
            .push(action)
    }
    pub fn run_action(&self, value: Value, action: Action) {
        match action {
            Action::DirectMap(at) => self.set_attr_notify(self.peer_id, &at, value),
            Action::Toggle(at) => {
                println!("gettt");
                if let Some(val) = self.get_attr(self.peer_id, &at) {
                    match val {
                        Value::Bool(val) => {
                            self.set_attr_notify(self.peer_id, &at, Value::Bool(!val));
                        }
                        Value::Number(val) => {
                            if val > 0.0 {
                                self.set_attr_notify(self.peer_id, &at, Value::Number(0.0));
                            } else {
                                self.set_attr_notify(self.peer_id, &at, Value::Number(1.0));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Action::Increase(_, _) => todo!(),
            Action::Decrease(_, _) => todo!(),
        }
    }

    pub fn get_attr(&self, peer: PeerId, name: &str) -> Option<Value> {
        self.attributes
            .borrow()
            .get(&peer)
            .map(|m| m.get(name))
            .flatten()
            .map(|a| a.value.clone())
    }
    pub fn set_attr_notify(&self, peer: PeerId, name: &str, data: Value) {
        self.set_attr(peer, name, data)
            .clone()
            .notify(usize::max_value());
    }
    pub fn set_attr(&self, peer: PeerId, name: &str, data: Value) -> Rc<Event> {
        let mut attributes = self.attributes.borrow_mut();
        let thi = attributes
            .entry(peer)
            .or_insert_with(|| BTreeMap::new())
            .entry(name.to_owned())
            .or_insert_with(|| Attribute {
                value: Value::Null,
                listener: Rc::new(Event::new()),
                actions: Vec::new(),
            });
        thi.value = data;
        Rc::clone(&thi.listener)
    }
    pub async fn set_attr_and_share(&self, topic: &str, data: Value) {
        self.set_attr(self.peer_id, topic, data.clone());
        for (_peerid, subscriber) in self.peers.borrow().iter() {
            subscriber
                .send(Message::Value(topic.to_owned(), data.clone()))
                .await
                .ok();
        }
    }
    pub async fn set_attr_notify_and_share(&self, topic: &str, data: Value) {
        self.set_attr(self.peer_id, topic, data.clone())
            .clone()
            .notify(usize::max_value());
        for (_peerid, subscriber) in self.peers.borrow().iter() {
            subscriber
                .send(Message::Value(topic.to_owned(), data.clone()))
                .await
                .ok();
        }
    }
    pub async fn request_set_attr(&self, peer: &PeerId, topic: &str, data: Value) {
        println!("1req: {peer}-{topic}-{:?}", data);
        if let Some(sender) = self.peers.borrow().get(peer) {
            println!("2req: {peer}-{topic}-{:?}", data);
            sender.send(Message::Set(topic.to_owned(), data)).await;
        }
    }
    pub async fn wait(&self, name: &str) -> Option<Value> {
        self.wait_peer(self.peer_id, name).await
    }
    pub async fn wait_peer(&self, peer: PeerId, name: &str) -> Option<Value> {
        let listener = {
            self.attributes
                .borrow_mut()
                .entry(peer)
                .or_insert_with(|| BTreeMap::new())
                .entry(name.to_owned())
                .or_insert_with(|| Attribute {
                    value: Value::Null,
                    listener: Rc::new(Event::new()),
                    actions: Vec::new(),
                })
                .listener
                .clone()
        };
        listener.listen().await;
        self.get_attr(peer, name)
    }
    pub fn backup(&self) -> Vec<u8> {
        postcard::to_allocvec(&*self.attributes.borrow()).unwrap()
    }
    pub fn restore(&self, data: &[u8]) -> anyhow::Result<()> {
        let mut restore = postcard::from_bytes(data)?;
        self.attributes.borrow_mut().append(&mut restore);
        Ok(())
    }
}

impl ServiceName for AttributesService {
    type Name = &'static str;
    fn name(&self) -> Self::Name {
        "attributes"
    }
}
impl<C: Channel + Authenticated> Service<C> for AttributesService {
    type Output = ();

    async fn upgrade(&self, channel: C) -> anyhow::Result<Self::Output> {
        let peerid = channel.peer_id();
        let (tx, rx) = bounded(5);
        let tx2 = tx.clone();
        self.peers.borrow_mut().insert(peerid, tx);
        if self.sync_new_peer.load(Relaxed) {
            channel.send_postcard(&Message::Sync).await?;
        }
        let task1 = async {
            while let Ok(msg) = rx.recv().await {
                channel.send_postcard(&msg).await?;
                println!("Sent {:?}", msg);
            }
            println!("Done attributes task1");
            Ok(()) as anyhow::Result<()>
        };

        let task2 = async {
            while let Ok(msg) = channel.recv_postcard::<Message>().await {
                println!("Received {:?}", msg);
                match msg {
                    Message::Value(key, value) => {
                        let actions = self
                            .attributes
                            .borrow_mut()
                            .entry(peerid)
                            .or_insert_with(|| BTreeMap::new())
                            .entry(key.clone())
                            .and_modify(|at| {
                                at.value = value.clone();
                                at.listener.notify(usize::MAX);
                            })
                            .or_insert_with(|| Attribute {
                                value: value.clone(),
                                actions: Vec::new(),
                                listener: Rc::new(Event::new()),
                            })
                            .actions
                            .iter()
                            .map(|a| a.clone())
                            .collect::<Vec<Action>>();
                        actions.iter().for_each(|action| {
                            self.run_action(value.clone(), action.clone());
                        });
                        if let Some(sender) = self.recv_any.borrow().as_ref() {
                            sender.send((peerid, key, value)).await;
                        }
                    }
                    Message::Sync => {
                        let attrs: Vec<(String, Value)> = self
                            .attributes
                            .borrow_mut()
                            .entry(self.peer_id)
                            .or_insert_with(|| BTreeMap::new())
                            .iter()
                            .map(|(k, v)| (k.clone(), v.value.clone()))
                            .collect();

                        for (k, v) in attrs {
                            tx2.send(Message::Value(k, v)).await.ok();
                            yield_now().await;
                        }
                    }
                    Message::Set(key, value) => {
                        self.set_attr_notify_and_share(&key, value).await;
                    }
                }
            }
            println!("Done attributes task2");
            Ok(())
        };
        futures_lite::future::or(task1, task2).await?;
        println!("Done attributes");
        Ok(())
    }
}
