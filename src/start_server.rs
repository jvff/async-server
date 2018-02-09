use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::{Async, Future, Poll, Sink, Stream};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Handle;
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::async_server_error::AsyncServerError;
use super::errors::{Error, ErrorKind};
use super::finite_service::FiniteService;
use super::listening_server::ListeningServer;

pub struct StartServer<S, P> {
    address: SocketAddr,
    service_factory: Option<S>,
    protocol: Arc<Mutex<P>>,
    handle: Handle,
}

impl<S, P> StartServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    S::Instance: FiniteService,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>
        + From<P::Error>,
{
    pub fn new(
        address: SocketAddr,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
        handle: Handle,
    ) -> Self {
        Self {
            address,
            protocol,
            handle,
            service_factory: Some(service_factory),
        }
    }

    pub fn shutdown(
        &mut self,
    ) -> Poll<(), AsyncServerError<S::Error, P::Error>> {
        if let Some(service_factory) = self.service_factory.take() {
            let mut service = service_factory.new_service()
                .map_err(AsyncServerError::ServiceCreationError)?;

            match service.force_stop() {
                Ok(()) => Ok(Async::Ready(())),
                Err(error) => {
                    Err(AsyncServerError::ServiceShutdownError(error))
                }
            }
        } else {
            Err(AsyncServerError::IncorrectShutdownInStartServer)
        }
    }

    fn start_server(&mut self) -> Poll<ListeningServer<S, P>, Error> {
        let listener = TcpListener::bind(&self.address, &self.handle)?;
        let protocol = self.protocol.clone();

        if let Some(service_factory) = self.service_factory.take() {
            Ok(Async::Ready(
                ListeningServer::new(listener, service_factory, protocol),
            ))
        } else {
            Err(ErrorKind::AttemptToStartServerTwice.into())
        }
    }
}

impl<S, P> Future for StartServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    S::Instance: FiniteService,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>
        + From<P::Error>,
{
    type Item = ListeningServer<S, P>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.service_factory.is_some() {
            self.start_server()
        } else {
            Err(ErrorKind::AttemptToStartServerTwice.into())
        }
    }
}
