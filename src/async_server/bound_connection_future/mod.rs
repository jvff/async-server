mod state;

use std::sync::{Arc, Mutex};

use futures::{Future, Poll};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_proto::pipeline::ServerProto;

use self::state::{State, WaitForConnection};
use super::connection_future::ConnectionFuture;
use super::errors::Error;

pub struct BoundConnectionFuture<P>
where
    P: ServerProto<TcpStream>,
{
    state: State<P>,
}

impl<P> BoundConnectionFuture<P>
where
    P: ServerProto<TcpStream>,
{
    pub fn from(listener: TcpListener, protocol: Arc<Mutex<P>>) -> Self {
        let connection = ConnectionFuture::from(listener);
        let state_data = WaitForConnection::from(connection, protocol);

        Self {
            state: State::WaitingForConnection(state_data),
        }
    }
}

impl<P> Future for BoundConnectionFuture<P>
where
    P: ServerProto<TcpStream>,
{
    type Item = P::Transport;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.state.advance()
    }
}
