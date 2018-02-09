use std::io;
use std::sync::{Arc, Mutex};

use futures::{Async, Future, Poll};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::active_server::ActiveServer;
use super::async_server_error::AsyncServerError;
use super::bound_connection_future::BoundConnectionFuture;
use super::finite_service::FiniteService;

pub struct ListeningServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService,
{
    connection: BoundConnectionFuture<P>,
    new_service: Option<io::Result<S::Instance>>,
}

impl<S, P> ListeningServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    S::Instance: FiniteService,
{
    pub fn new(
        listener: TcpListener,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
    ) -> Self {
        ListeningServer {
            new_service: Some(service_factory.new_service()),
            connection: BoundConnectionFuture::from(listener, protocol),
        }
    }

    pub fn shutdown(
        &mut self,
    ) -> Poll<(), AsyncServerError<S::Error, P::Error>> {
        let mut service =
            self.service(AsyncServerError::IncorrectShutdownInListeningServer)?;

        service.force_stop()
            .map(Async::Ready)
            .map_err(AsyncServerError::ServiceShutdownError)
    }

    fn service(
        &mut self,
        empty_service_error: AsyncServerError<S::Error, P::Error>,
    ) -> Result<S::Instance, AsyncServerError<S::Error, P::Error>> {
        let new_service_result =
            self.new_service.take().ok_or(empty_service_error)?;

        new_service_result.map_err(AsyncServerError::ServiceCreationError)
    }
}

impl<S, P> Future for ListeningServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    S::Instance: FiniteService,
{
    type Item = ActiveServer<S::Instance, P::Transport>;
    type Error = AsyncServerError<S::Error, P::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let connection = try_ready!(
            self.connection.poll().map_err(AsyncServerError::BindError)
        );

        let service = self.service(AsyncServerError::ListenedTwice)?;

        Ok(Async::Ready(ActiveServer::new(connection, service)))
    }
}
