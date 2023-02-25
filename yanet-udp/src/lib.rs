#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

use std::{
    cell::RefCell,
    io::{self, Error, ErrorKind, Result},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use yanet_core::Socket;

pub async fn try_async<T>(mut f: impl FnMut() -> Result<T>) -> Result<T> {
    loop {
        match f() {
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                futures_timer::Delay::new(Duration::from_millis(100)).await;
            }
            res => return res,
        }
    }
}

pub struct Udp {
    peers: RefCell<Vec<SocketAddr>>,
    inner: UdpSocket,
}

impl Udp {
    pub fn new<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        Ok(Self {
            peers: Default::default(),
            inner: socket,
        })
    }
    pub fn join_multicast_v4(&self, multicast: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.peers
            .borrow_mut()
            .push(SocketAddrV4::new(multicast.clone(), self.inner.local_addr()?.port()).into());
        self.inner.join_multicast_v4(multicast, interface)?;
        Ok(())
    }
    pub fn add_peer<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        addr.to_socket_addrs()?
            .for_each(|a| self.peers.borrow_mut().push(a));
        Ok(())
    }
}
impl Socket for Udp {
    type Addr = SocketAddr;
    type Error = Error;

    async fn broadcast(&self, buf: &[u8]) -> std::result::Result<(), Self::Error> {
        for addr in self.peers.borrow().clone().iter() {
            self.send(buf, *addr).await?;
        }
        Ok(())
    }

    async fn send(&self, buf: &[u8], addr: Self::Addr) -> Result<usize> {
        try_async(|| self.inner.send_to(buf, addr)).await
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, Self::Addr)> {
        try_async(|| self.inner.recv_from(buf)).await
    }
}
