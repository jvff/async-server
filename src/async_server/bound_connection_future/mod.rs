mod state;

use std::sync::{Arc, Mutex};

use futures::{Future, Poll};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_proto::pipeline::ServerProto;

use self::state::State;
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

        Self {
            state: State::start_with(connection, protocol),
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
