//! Horizon's pollable descriptors are BSD sockets; a UDP loopback pair wakes poll.
use std::io;
use std::net::UdpSocket;
use std::os::fd::{AsRawFd, RawFd};
use crate::sys::Selector;
use crate::{Interest, Token};

#[derive(Debug)]
pub(crate) struct Waker { sender: UdpSocket, receiver: UdpSocket }
impl Waker {
    #[allow(dead_code)]
    pub(crate) fn new(selector: &Selector, token: Token) -> io::Result<Self> {
        let waker = Self::new_unregistered()?;
        selector.register(waker.as_raw_fd(), token, Interest::READABLE)?;
        Ok(waker)
    }
    pub(crate) fn new_unregistered() -> io::Result<Self> {
        let receiver = UdpSocket::bind("127.0.0.1:0")?;
        receiver.set_nonblocking(true)?;
        let sender = UdpSocket::bind("127.0.0.1:0")?;
        sender.connect(receiver.local_addr()?)?;
        sender.set_nonblocking(true)?;
        Ok(Self { sender, receiver })
    }
    pub(crate) fn wake(&self) -> io::Result<()> {
        match self.sender.send(&[1]) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(e),
        }
    }
    pub(crate) fn ack_and_reset(&self) {
        let mut bytes = [0u8; 128];
        loop {
            match self.receiver.recv(&mut bytes) {
                Ok(_) => {},
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    }
}
impl AsRawFd for Waker { fn as_raw_fd(&self) -> RawFd { self.receiver.as_raw_fd() } }
