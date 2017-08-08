use std::sync::{Arc, Mutex};

use futures::{Async, Future, Poll, Sink, Stream};
use futures::future::{FutureResult, Join};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::active_async_server::ActiveAsyncServer;
use super::bound_connection_future::BoundConnectionFuture;
use super::errors::{Error, NormalizeError};

pub struct ListeningAsyncServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService,
    Error: From<S::Error> + From<P::Error>,
{
    connection_and_service: Join<
        BoundConnectionFuture<P>,
        FutureResult<S::Instance, Error>,
    >,
}

impl<S, P> ListeningAsyncServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
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
        let service = service_factory.new_service();
        let connection = BoundConnectionFuture::from(listener, protocol);

        ListeningAsyncServer {
            connection_and_service: connection.join(service.normalize_error()),
        }
    }
}

impl<S, P> Future for ListeningAsyncServer<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>
        + From<P::Error>,
{
    type Item = ActiveAsyncServer<S::Instance, P::Transport>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let (connection, service) =
            try_ready!(self.connection_and_service.poll());

        Ok(Async::Ready(ActiveAsyncServer::new(connection, service)))
    }
}
