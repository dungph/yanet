#![feature(async_fn_in_trait)]
use std::{
    cell::RefCell,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    time::Duration,
};

use async_channel::{bounded, Receiver, Sender};
use yanet_core::{Channel, Transport};

type Pair<T> = (Sender<T>, Receiver<T>);
pub struct TcpTransport {
    incoming: Pair<(bool, TcpStream)>,
}

impl TcpTransport {
    pub fn new() -> Self {
        Self {
            incoming: bounded(1),
        }
    }
    pub async fn connect<A: ToSocketAddrs>(&self, addr: A) -> anyhow::Result<()> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        self.incoming.0.send((true, stream)).await?;
        Ok(())
    }
    pub async fn listen<A: ToSocketAddrs>(&self, addr: A) -> anyhow::Result<()> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        loop {
            let (stream, _addr) = listener.accept()?;
            stream.set_nonblocking(true)?;
            self.incoming.0.send((false, stream)).await?;
        }
    }
}

impl Transport for TcpTransport {
    type Channel = TcpChannel;

    async fn get(&self) -> anyhow::Result<Option<Self::Channel>> {
        let (is_initiator, socket) = self.incoming.1.recv().await?;
        Ok(Some(TcpChannel {
            is_initiator,
            socket: RefCell::new(socket),
        }))
    }
}
pub struct TcpChannel {
    is_initiator: bool,
    socket: RefCell<TcpStream>,
}

impl Channel for TcpChannel {
    fn is_initiator(&self) -> bool {
        self.is_initiator
    }

    async fn recv(&self) -> anyhow::Result<Vec<u8>> {
        let mut buf = [0u8; 1024];
        try_async(|| self.socket.borrow_mut().read_exact(&mut buf[..2])).await?;
        let len = u16::from_be_bytes(buf[..2].try_into().unwrap()) as usize;

        try_async(|| self.socket.borrow_mut().read_exact(&mut buf[..len])).await?;
        Ok(buf[..len].to_owned())
    }

    async fn send(&self, buf: &[u8]) -> anyhow::Result<()> {
        let len = buf.len() as u16;
        try_async(|| {
            self.socket
                .borrow_mut()
                .write_all(len.to_be_bytes().as_slice())
        })
        .await?;
        try_async(|| self.socket.borrow_mut().write_all(buf)).await?;
        todo!()
    }
}

pub async fn try_async<T>(mut f: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        match f() {
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                futures_timer::Delay::new(Duration::from_millis(100)).await;
            }
            res => return res,
        }
    }
}
