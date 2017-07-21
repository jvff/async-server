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
use super::errors::{Error, NormalizeError};

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
        address: SocketAddr,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
        handle: Handle,
    ) -> Self {
        let state =
            State::start_with(address, service_factory, protocol, handle);

        Self { state }
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

enum State<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>,
{
    WaitingToStart(WaitToStart<S, P>),
    WaitingForParameters(WaitForParameters<S, P>),
    ServerReady(ServerReady<S, P>),
    Processing,
}

impl<S, P> State<S, P>
where
    P: ServerProto<TcpStream>,
    S: Clone + NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>,
{
    fn start_with(
        address: SocketAddr,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
        handle: Handle,
    ) -> Self {
        let wait_to_start =
            WaitToStart::new(address, service_factory, protocol, handle);

        State::WaitingToStart(wait_to_start)
    }

    fn advance(&mut self) -> Poll<(), Error> {
        let state = mem::replace(self, State::Processing);

        let (poll_result, new_state) = state.advance_to_new_state();

        mem::replace(self, new_state);

        poll_result
    }

    fn advance_to_new_state(self) -> (Poll<(), Error>, Self) {
        match self {
            State::WaitingToStart(handler) => handler.advance(),
            State::WaitingForParameters(handler) => handler.advance(),
            State::ServerReady(handler) => handler.advance(),
            State::Processing => panic!("State has more than one owner"),
        }
    }
}

struct WaitToStart<S, P> {
    address: SocketAddr,
    service_factory: S,
    protocol: Arc<Mutex<P>>,
    handle: Handle,
}


impl<S, P> WaitToStart<S, P>
where
    P: ServerProto<TcpStream>,
    S: Clone + NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>,
{
    fn new(
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
        }
    }

    fn advance(self) -> (Poll<(), Error>, State<S, P>) {
        let bind_result = TcpListener::bind(&self.address, &self.handle);

        match bind_result {
            Ok(listener) => self.create_server_parameters(listener),
            Err(error) => (Err(error.into()), self.same_state()),
        }
    }

    fn create_server_parameters(
        self,
        listener: TcpListener,
    ) -> (Poll<(), Error>, State<S, P>) {
        let service_factory = self.service_factory.clone();
        let protocol = self.protocol.clone();

        WaitForParameters::advance_with(listener, service_factory, protocol)
    }

    fn same_state(self) -> State<S, P> {
        State::WaitingToStart(self)
    }
}

struct WaitForParameters<S, P>
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
    fn advance_with(
        listener: TcpListener,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
    ) -> (Poll<(), Error>, State<S, P>) {
        let service = service_factory.new_service();
        let connection = BoundConnectionFuture::from(listener, protocol);
        let parameters = connection.join(service.normalize_error());

        let wait_for_parameters = WaitForParameters { parameters };

        wait_for_parameters.advance()
    }

    fn advance(mut self) -> (Poll<(), Error>, State<S, P>) {
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

struct ServerReady<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
{
    server: ActiveAsyncServer<S::Instance, P::Transport>,
}

impl<S, P> ServerReady<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>,
{
    fn advance_with(
        parameters_tuple: (P::Transport, S::Instance),
    ) -> (Poll<(), Error>, State<S, P>) {
        let server_ready = Self {
            server: ActiveAsyncServer::from_tuple(parameters_tuple),
        };

        server_ready.advance()
    }

    fn advance(mut self) -> (Poll<(), Error>, State<S, P>) {
        (self.server.poll(), State::ServerReady(self))
    }
}
