use std::mem;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::{Future, Poll, Sink, Stream};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::wait_for_parameters::WaitForParameters;
use super::wait_to_start::WaitToStart;
use super::super::active_async_server::ActiveAsyncServer;
use super::super::errors::Error;

pub enum State<S, P>
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
    pub fn start_with(
        address: SocketAddr,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
        handle: Handle,
    ) -> Self {
        let wait_to_start =
            WaitToStart::new(address, service_factory, protocol, handle);

        State::WaitingToStart(wait_to_start)
    }

    pub fn advance(&mut self) -> Poll<(), Error> {
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

pub struct ServerReady<S, P>
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
    pub fn advance_with(
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
