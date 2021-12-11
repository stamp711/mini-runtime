use std::io::{self, Read, Write};
use std::net::{
    SocketAddr, TcpListener as StdTcpListener, TcpStream as StdTcpStream, ToSocketAddrs,
};
use std::os::unix::prelude::AsRawFd;
use std::task::Poll;

use futures::{AsyncRead, AsyncWrite, Stream};

use crate::reactor::{self, REACTOR};

pub struct TcpListener {
    std_listener: StdTcpListener,
}

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let addrs = addr.to_socket_addrs()?;

        let mut last_err = None;

        for addr in addrs {
            match TcpListener::bind_addr(addr) {
                Ok(listener) => return Ok(listener),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
        }))
    }

    fn bind_addr(addr: SocketAddr) -> io::Result<Self> {
        let std_listener = StdTcpListener::bind(addr)?;
        REACTOR.with(|reactor| reactor.borrow().add(&std_listener));
        Ok(Self { std_listener })
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        REACTOR.with(|reactor| reactor.borrow_mut().delete(&self.std_listener));
    }
}

impl Stream for TcpListener {
    type Item = io::Result<(TcpStream, SocketAddr)>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.std_listener.accept() {
            Ok((std_stream, addr)) => {
                println!("[tcp_listener] accept from {}", addr);
                Poll::Ready(Some(Ok((std_stream.into(), addr))))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                println!(
                    "[tcp_listener] accept() would block, registering interest in {:?} readable to reactor",
                    self.std_listener
                );
                // register interest to reactor
                REACTOR.with(|reactor| {
                    reactor
                        .borrow_mut()
                        .wake_on_readable(&self.std_listener, cx)
                });
                Poll::Pending
            }
            Err(e) => {
                println!("[tcp_listener] error: {:?}", e);
                Poll::Ready(Some(Err(e)))
            }
        }
    }
}

pub struct TcpStream {
    std_stream: StdTcpStream,
}

impl From<StdTcpStream> for TcpStream {
    fn from(std_stream: StdTcpStream) -> Self {
        REACTOR.with(|reactor| reactor.borrow().add(&std_stream));
        Self { std_stream }
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        REACTOR.with(|reactor| reactor.borrow_mut().delete(&self.std_stream));
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        match self.std_stream.read(buf) {
            Ok(n) => {
                println!("[tcp_stream] read {} bytes", n);
                Poll::Ready(Ok(n))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                println!(
                    "[tcp_stream] read would block, registering interest in {:?} readable to reactor",
                    self.std_stream
                );
                // register interest to reactor
                REACTOR.with(|reactor| reactor.borrow_mut().wake_on_readable(&self.std_stream, cx));
                Poll::Pending
            }
            Err(e) => {
                println!("[tcp_stream] read error: {:?}", e);
                Poll::Ready(Err(e))
            }
        }
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        match self.std_stream.write(buf) {
            Ok(n) => {
                println!("[tcp_stream] write {} bytes", n);
                Poll::Ready(Ok(n))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                println!(
                    "[tcp_stream] write would block, registering interest in {:?} writable to reactor",
                    self.std_stream
                );
                // register interest to reactor
                REACTOR.with(|reactor| reactor.borrow_mut().wake_on_writable(&self.std_stream, cx));
                Poll::Pending
            }
            Err(e) => {
                println!("[tcp_stream] write error: {:?}", e);
                Poll::Ready(Err(e))
            }
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        self.std_stream.shutdown(std::net::Shutdown::Write)?;
        Poll::Ready(Ok(()))
    }
}
