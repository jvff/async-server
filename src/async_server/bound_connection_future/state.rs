use std::mem;
use std::sync::{Arc, Mutex, MutexGuard};

use futures::{Async, Future, IntoFuture, Poll};
use tokio_core::net::TcpStream;
use tokio_proto::pipeline::ServerProto;

use super::super::connection_future::ConnectionFuture;
use super::super::errors::{Error, ErrorKind};

pub enum State<P>
where
    P: ServerProto<TcpStream>,
{
    Processing,
    WaitingForConnection(WaitForConnection<P>),
    WaitingForBindResult(WaitForBindResult<P>),
    Finished,
}

impl<P> State<P>
where
    P: ServerProto<TcpStream>,
{
    pub fn start_with(
        connection: ConnectionFuture,
        protocol: Arc<Mutex<P>>,
    ) -> Self {
        let state_data = WaitForConnection::from(connection, protocol);

        State::WaitingForConnection(state_data)
    }

    pub fn advance(&mut self) -> Poll<P::Transport, Error> {
        let state = mem::replace(self, State::Processing);

        let (poll_result, new_state) = state.advance_to_new_state();

        mem::replace(self, new_state);

        poll_result
    }

    fn advance_to_new_state(self) -> (Poll<P::Transport, Error>, Self) {
        match self {
            State::WaitingForConnection(handler) => handler.advance(),
            State::WaitingForBindResult(handler) => handler.advance(),
            State::Processing => panic!("State has more than one owner"),
            State::Finished => panic!("Future called twice"),
        }
    }
}

pub struct WaitForConnection<P> {
    connection: ConnectionFuture,
    protocol: Arc<Mutex<P>>,
}

impl<P> WaitForConnection<P>
where
    P: ServerProto<TcpStream>,
{
    pub fn from(connection: ConnectionFuture, protocol: Arc<Mutex<P>>) -> Self {
        Self {
            connection,
            protocol,
        }
    }

    fn advance(mut self) -> (Poll<P::Transport, Error>, State<P>) {
        match self.connection.poll() {
            Ok(Async::Ready((socket, _))) => self.bind_connection(socket),
            Ok(Async::NotReady) => (Ok(Async::NotReady), self.same_state()),
            Err(error) => (Err(error), self.same_state()),
        }
    }

    fn bind_connection(
        self,
        socket: TcpStream,
    ) -> (Poll<P::Transport, Error>, State<P>) {
        let result = if let Ok(protocol) = self.protocol.lock() {
            Some(WaitForBindResult::advance_with(protocol, socket))
        } else {
            None
        };

        if let Some(result) = result {
            result
        } else {
            self.bind_connection_failure()
        }
    }

    fn bind_connection_failure(self) -> (Poll<P::Transport, Error>, State<P>) {
        let bind_error: Error = ErrorKind::FailedToBindConnection.into();

        (Err(bind_error), self.same_state())
    }

    fn same_state(self) -> State<P> {
        State::WaitingForConnection(self)
    }
}

pub struct WaitForBindResult<P>
where
    P: ServerProto<TcpStream>,
{
    bind_result: <P::BindTransport as IntoFuture>::Future,
}

impl<P> WaitForBindResult<P>
where
    P: ServerProto<TcpStream>,
{
    fn advance_with(
        protocol: MutexGuard<P>,
        socket: TcpStream,
    ) -> (Poll<P::Transport, Error>, State<P>) {
        let bind_result = protocol.bind_transport(socket).into_future();
        let bind_future = WaitForBindResult { bind_result };

        bind_future.advance()
    }

    fn advance(mut self) -> (Poll<P::Transport, Error>, State<P>) {
        match self.bind_result.poll() {
            Ok(Async::Ready(bound_connection)) => self.finish(bound_connection),
            Ok(Async::NotReady) => (Ok(Async::NotReady), self.same_state()),
            Err(error) => (Err(error.into()), self.same_state()),
        }
    }

    fn finish(
        self,
        connection: P::Transport,
    ) -> (Poll<P::Transport, Error>, State<P>) {
        (Ok(Async::Ready(connection)), State::Finished)
    }

    fn same_state(self) -> State<P> {
        State::WaitingForBindResult(self)
    }
}
