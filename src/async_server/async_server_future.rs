use std::mem;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::{Async, Future, Poll, Sink, Stream};
use futures::future::{FutureResult, Join};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Handle;
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::active_async_server::ActiveAsyncServer;
use super::bound_connection_future::BoundConnectionFuture;
use super::errors::{Error, NormalizeError, Result};

type ServerParametersFuture<S, P> = Join<
    BoundConnectionFuture<P>,
    FutureResult<S, Error>,
>;

pub struct AsyncServerFuture<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
{
    address: SocketAddr,
    service_factory: S,
    protocol: Arc<Mutex<P>>,
    handle: Handle,
    server_parameters: Option<Result<ServerParametersFuture<S::Instance, P>>>,
    server: Option<ActiveAsyncServer<S::Instance, P::Transport>>,
}

impl<S, P> AsyncServerFuture<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>,
{
    pub fn new(
        address: SocketAddr,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
        handle: Handle,
    ) -> Self {
        Self {
            address,
            service_factory,
            protocol,
            handle,
            server_parameters: None,
            server: None,
        }
    }

    fn start_listening(&mut self) -> Poll<(), Error> {
        let bind_result = TcpListener::bind(&self.address, &self.handle);

        self.server_parameters = match bind_result {
            Ok(listener) => Some(Ok(self.create_server_parameters(listener))),
            Err(error) => Some(Err(error.into())),
        };

        self.poll_server_parameters()
    }

    fn create_server_parameters(
        &mut self,
        listener: TcpListener,
    ) -> ServerParametersFuture<S::Instance, P> {
        let service = self.service_factory.new_service();
        let protocol = self.protocol.clone();
        let connection = BoundConnectionFuture::from(listener, protocol);

        connection.join(service.normalize_error())
    }

    fn poll_server_parameters(&mut self) -> Poll<(), Error> {
        let parameters = mem::replace(&mut self.server_parameters, None);

        let (poll_result, maybe_parameters) = match parameters {
            Some(Ok(mut parameters)) => {
                (parameters.poll(), Some(Ok(parameters)))
            }
            Some(Err(error)) => (Err(error), None),
            None => {
                panic!(
                    "Attempt to poll server parameters future before it is \
                     created"
                )
            }
        };

        mem::replace(&mut self.server_parameters, maybe_parameters);

        match poll_result {
            Ok(Async::Ready(parameters)) => self.start_server(parameters),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(error) => Err(error),
        }
    }

    fn start_server(
        &mut self,
        parameters: (P::Transport, S::Instance),
    ) -> Poll<(), Error> {
        self.server = Some(ActiveAsyncServer::from_tuple(parameters));

        self.poll_server()
    }

    fn poll_server(&mut self) -> Poll<(), Error> {
        match self.server {
            Some(ref mut server) => server.poll(),
            None => {
                panic!("Attempt to poll server future before it is created")
            }
        }
    }

    fn state(&self) -> State {
        if self.server.is_some() {
            State::ServerReady
        } else if self.server_parameters.is_some() {
            State::WaitingForParameters
        } else {
            State::NoParameters
        }
    }
}

impl<S, P> Future for AsyncServerFuture<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.state() {
            State::NoParameters => self.start_listening(),
            State::WaitingForParameters => self.poll_server_parameters(),
            State::ServerReady => self.poll_server(),
        }
    }
}

enum State {
    NoParameters,
    WaitingForParameters,
    ServerReady,
}
