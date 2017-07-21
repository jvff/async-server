use futures::{Future, Poll, Sink, Stream};
use tokio_core::net::TcpStream;
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::state::State;
use super::super::active_async_server::ActiveAsyncServer;
use super::super::errors::Error;

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

    pub fn advance(mut self) -> (Poll<(), Error>, State<S, P>) {
        (self.server.poll(), State::ServerReady(self))
    }
}
