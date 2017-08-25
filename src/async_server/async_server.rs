use std::mem;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::{Async, Future, Poll};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::active_server::ActiveServer;
use super::errors::{Error, ErrorKind};
use super::finite_service::FiniteService;
use super::listening_server::ListeningServer;
use super::start_server::StartServer;

pub enum AsyncServer<S, P>
where
    S: NewService,
    P: ServerProto<TcpStream>,
    S::Instance: FiniteService,
    Error: From<P::Error> + From<S::Error>,
{
    Binding(StartServer<S, P>),
    BindCancelled(StartServer<S, P>),
    Listening(ListeningServer<S, P>),
    ListenCancelled(ListeningServer<S, P>),
    Active(ActiveServer<S::Instance, P::Transport>),
    Disconnecting(ActiveServer<S::Instance, P::Transport>),
    Dead,
}

impl<S, P> AsyncServer<S, P>
where
    S: NewService<Request = P::Request, Response = P::Response>,
    P: ServerProto<TcpStream>,
    S::Instance: FiniteService,
    Error: From<P::Error> + From<S::Error>,
{
    pub fn new(
        address: SocketAddr,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
        handle: Handle,
    ) -> Self {
        AsyncServer::Binding(
            StartServer::new(address, service_factory, protocol, handle),
        )
    }

    pub fn shutdown(&mut self) -> Poll<(), Error> {
        let shutdown_result = match *self {
            AsyncServer::Binding(ref mut handler) => handler.shutdown(),
            AsyncServer::BindCancelled(ref mut handler) => {
                return handler.shutdown();
            }
            AsyncServer::Listening(ref mut handler) => handler.shutdown(),
            AsyncServer::ListenCancelled(ref mut handler) => {
                return handler.shutdown();
            }
            AsyncServer::Active(ref mut handler) => handler.shutdown(),
            AsyncServer::Disconnecting(ref mut handler) => {
                return handler.shutdown();
            }
            AsyncServer::Dead => Ok(Async::Ready(())),
        };

        let new_state = match shutdown_result {
            Ok(Async::NotReady) => {
                match mem::replace(self, AsyncServer::Dead) {
                    AsyncServer::Binding(handler) => {
                        AsyncServer::BindCancelled(handler)
                    }
                    AsyncServer::Listening(handler) => {
                        AsyncServer::ListenCancelled(handler)
                    }
                    AsyncServer::Active(handler) => {
                        AsyncServer::Disconnecting(handler)
                    }
                    AsyncServer::Dead => AsyncServer::Dead,
                    shutting_down_state => shutting_down_state,
                }
            }
            _ => AsyncServer::Dead,
        };

        mem::replace(self, new_state);

        shutdown_result
    }
}

impl<S, P> From<StartServer<S, P>> for AsyncServer<S, P>
where
    S: NewService<Request = P::Request, Response = P::Response>,
    P: ServerProto<TcpStream>,
    S::Instance: FiniteService,
    Error: From<P::Error> + From<S::Error>,
{
    fn from(start_server: StartServer<S, P>) -> Self {
        AsyncServer::Binding(start_server)
    }
}

impl<S, P> From<ListeningServer<S, P>> for AsyncServer<S, P>
where
    S: NewService<Request = P::Request, Response = P::Response>,
    P: ServerProto<TcpStream>,
    S::Instance: FiniteService,
    Error: From<P::Error> + From<S::Error>,
{
    fn from(listening_server: ListeningServer<S, P>) -> Self {
        AsyncServer::Listening(listening_server)
    }
}

impl<S, P> From<ActiveServer<S::Instance, P::Transport>> for AsyncServer<S, P>
where
    S: NewService<Request = P::Request, Response = P::Response>,
    P: ServerProto<TcpStream>,
    S::Instance: FiniteService,
    Error: From<P::Error> + From<S::Error>,
{
    fn from(active_server: ActiveServer<S::Instance, P::Transport>) -> Self {
        AsyncServer::Active(active_server)
    }
}

impl<S, P> Future for AsyncServer<S, P>
where
    S: NewService<Request = P::Request, Response = P::Response>,
    P: ServerProto<TcpStream>,
    S::Instance: FiniteService,
    Error: From<P::Error> + From<S::Error>,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let maybe_new_state = match *self {
            AsyncServer::Binding(ref mut handler) => {
                Some(AsyncServer::Listening(try_ready!(handler.poll())))
            }
            AsyncServer::Listening(ref mut handler) => {
                Some(AsyncServer::Active(try_ready!(handler.poll())))
            }
            AsyncServer::Active(ref mut handler) => {
                try_ready!(handler.poll());
                None
            }
            AsyncServer::Dead => {
                return Err(ErrorKind::AsyncServerWasShutDown.into());
            }
            _ => return Err(ErrorKind::AsyncServerIsShuttingDown.into()),
        };

        if let Some(new_state) = maybe_new_state {
            mem::replace(self, new_state);
            self.poll()
        } else {
            Ok(Async::Ready(()))
        }
    }
}
