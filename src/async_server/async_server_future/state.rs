use std::mem;
use std::sync::{Arc, Mutex};

use futures::{Poll, Sink, Stream};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::server_ready::ServerReady;
use super::wait_for_parameters::WaitForParameters;
use super::super::errors::Error;

pub enum State<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>,
{
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
        listener: TcpListener,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
    ) -> Self {
        let wait_for_parameters =
            WaitForParameters::new(listener, service_factory, protocol);

        State::WaitingForParameters(wait_for_parameters)
    }

    pub fn advance(&mut self) -> Poll<(), Error> {
        let state = mem::replace(self, State::Processing);

        let (poll_result, new_state) = state.advance_to_new_state();

        mem::replace(self, new_state);

        poll_result
    }

    fn advance_to_new_state(self) -> (Poll<(), Error>, Self) {
        match self {
            State::WaitingForParameters(handler) => handler.advance(),
            State::ServerReady(handler) => handler.advance(),
            State::Processing => panic!("State has more than one owner"),
        }
    }
}
