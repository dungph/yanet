#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

use std::{
    cell::RefCell,
    fmt::Debug,
    io::{self, Error, ErrorKind, Result},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use serde::{de::DeserializeOwned, Serialize};
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
    pub fn new<A: ToSocketAddrs + Debug>(addr: A) -> io::Result<Self> {
        let socket = UdpSocket::bind(&addr)?;
        socket.set_nonblocking(true)?;
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

    async fn broadcast<D>(&self, data: &D) -> std::result::Result<(), Self::Error>
    where
        D: Serialize,
    {
        for addr in self.peers.borrow().clone().iter() {
            self.send(&data, *addr).await?;
        }
        Ok(())
    }

    async fn send<D>(&self, data: &D, addr: Self::Addr) -> std::result::Result<(), Self::Error>
    where
        D: Serialize + ?Sized,
    {
        let dat =
            postcard::to_allocvec(data).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        try_async(|| self.inner.send_to(&dat, addr)).await?;
        Ok(())
    }

    async fn recv<D>(&self) -> std::result::Result<(D, Self::Addr), Self::Error>
    where
        D: DeserializeOwned,
    {
        let mut buf = [0u8; 1024];
        let (len, addr) = try_async(|| self.inner.recv_from(&mut buf)).await?;
        Ok((
            postcard::from_bytes(&buf[..len])
                .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?,
            addr,
        ))
    }
}
