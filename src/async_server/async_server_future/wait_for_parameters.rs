use std::sync::{Arc, Mutex};

use futures::{Async, Future, Poll, Sink, Stream};
use futures::future::{FutureResult, Join};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::server_ready::ServerReady;
use super::state::State;
use super::super::bound_connection_future::BoundConnectionFuture;
use super::super::errors::{Error, NormalizeError};

pub struct WaitForParameters<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService,
    Error: From<S::Error>,
{
    parameters: Join<
        BoundConnectionFuture<P>,
        FutureResult<S::Instance, Error>,
    >,
}

impl<S, P> WaitForParameters<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>,
{
    pub fn new(
        listener: TcpListener,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
    ) -> Self {
        let service = service_factory.new_service();
        let connection = BoundConnectionFuture::from(listener, protocol);
        let parameters = connection.join(service.normalize_error());

        WaitForParameters { parameters }
    }

    pub fn advance(mut self) -> (Poll<(), Error>, State<S, P>) {
        match self.parameters.poll() {
            Ok(Async::Ready(parameters)) => self.create_server(parameters),
            Ok(Async::NotReady) => (Ok(Async::NotReady), self.same_state()),
            Err(error) => (Err(error), self.same_state()),
        }
    }

    fn create_server(
        self,
        parameters_tuple: (P::Transport, S::Instance),
    ) -> (Poll<(), Error>, State<S, P>) {
        ServerReady::advance_with(parameters_tuple)
    }

    fn same_state(self) -> State<S, P> {
        State::WaitingForParameters(self)
    }
}
