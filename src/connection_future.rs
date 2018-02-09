use std::net::SocketAddr;

use futures::{Async, Future, Poll, Stream};
use tokio_core::net::{Incoming, TcpListener, TcpStream};

use super::connection_error::ConnectionError;

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
    type Error = ConnectionError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        use super::connection_error::ConnectionError::*;

        match self.incoming_connections.poll() {
            Ok(Async::Ready(Some(connection))) => Ok(Async::Ready(connection)),
            Ok(Async::Ready(None)) => Err(NoConnectionsReceived),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(cause) => Err(FailedToReceiveConnection(cause)),
        }
    }
}
