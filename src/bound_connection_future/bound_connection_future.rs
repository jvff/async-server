use std::sync::{Arc, Mutex};

use futures::{Future, Poll};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_proto::pipeline::ServerProto;

use super::bind_connection_error::BindConnectionError;
use super::state::State;
use super::super::connection_future::ConnectionFuture;

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
    type Error = BindConnectionError<P::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.state.advance()
    }
}
