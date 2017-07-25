use std::sync::{Arc, Mutex};

use futures::{Future, Poll, Sink, Stream};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::state::State;
use super::super::errors::Error;

pub struct AsyncServerFuture<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>,
{
    state: State<S, P>,
}

impl<S, P> AsyncServerFuture<S, P>
where
    P: ServerProto<TcpStream>,
    S: Clone + NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>,
{
    pub fn new(
        listener: TcpListener,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
    ) -> Self {
        AsyncServerFuture {
            state: State::start_with(listener, service_factory, protocol),
        }
    }
}

impl<S, P> Future for AsyncServerFuture<S, P>
where
    P: ServerProto<TcpStream>,
    S: Clone + NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.state.advance()
    }
}
