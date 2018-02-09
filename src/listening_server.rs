use std::{io, mem};
use std::sync::{Arc, Mutex};

use futures::{Async, Future, Poll, Sink, Stream};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::active_server::ActiveServer;
use super::async_server_error::AsyncServerError;
use super::bound_connection_future::BoundConnectionFuture;
use super::errors::{Error, ErrorKind};
use super::finite_service::FiniteService;

pub struct ListeningServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService,
    Error: From<S::Error> + From<P::Error>,
{
    connection: BoundConnectionFuture<P>,
    service: io::Result<S::Instance>,
}

impl<S, P> ListeningServer<S, P>
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
        listener: TcpListener,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
    ) -> Self {
        ListeningServer {
            service: service_factory.new_service(),
            connection: BoundConnectionFuture::from(listener, protocol),
        }
    }

    pub fn shutdown(
        &mut self,
    ) -> Poll<(), AsyncServerError<S::Error, P::Error>> {
        if let Ok(ref mut service) = self.service {
            match service.force_stop() {
                Ok(()) => Ok(Async::Ready(())),
                Err(error) => {
                    Err(AsyncServerError::ServiceShutdownError(error))
                }
            }
        } else {
            Err(AsyncServerError::IncorrectShutdownInListeningServer)
        }
    }
}

impl<S, P> Future for ListeningServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    S::Instance: FiniteService,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>
        + From<P::Error>,
{
    type Item = ActiveServer<S::Instance, P::Transport>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let connection = try_ready!(
            self.connection.poll().map_err(|_| {
                Error::from(ErrorKind::FailedToReceiveConnection)
            })
        );

        let service = mem::replace(
            &mut self.service,
            Err(io::Error::new(
                io::ErrorKind::Other,
                "server listening state can't be polled for two connections",
            )),
        );

        Ok(Async::Ready(ActiveServer::new(connection, service?)))
    }
}
