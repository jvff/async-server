use std::net::SocketAddr;

use futures::{Async, Future, Poll, Stream};
use tokio_core::net::{Incoming, TcpListener, TcpStream};

use super::errors::{Error, ErrorKind};

pub struct ConnectionFuture {
    incoming_connections: Incoming,
}

impl ConnectionFuture {
    pub fn from(listener: TcpListener) -> Self {
        Self {
            incoming_connections: listener.incoming(),
        }
    }
}

impl Future for ConnectionFuture {
    type Item = (TcpStream, SocketAddr);
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.incoming_connections.poll() {
            Ok(Async::Ready(Some(connection))) => Ok(Async::Ready(connection)),
            Ok(Async::Ready(None)) => Err(
                ErrorKind::FailedToReceiveConnection.into(),
            ),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(error) => Err(error.into()),
        }
    }
}
