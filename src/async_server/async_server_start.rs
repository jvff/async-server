use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::{Async, Future, Poll, Sink, Stream};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Handle;
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::errors::{Error, ErrorKind};
use super::listening_async_server::ListeningAsyncServer;

pub struct AsyncServerStart<S, P> {
    address: SocketAddr,
    service_factory: Option<S>,
    protocol: Arc<Mutex<P>>,
    handle: Handle,
}

impl<S, P> AsyncServerStart<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
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
        Self {
            address,
            protocol,
            handle,
            service_factory: Some(service_factory),
        }
    }

    fn start_server(&mut self) -> Poll<ListeningAsyncServer<S, P>, Error> {
        let listener = TcpListener::bind(&self.address, &self.handle)?;
        let protocol = self.protocol.clone();

        if let Some(service_factory) = self.service_factory.take() {
            Ok(Async::Ready(ListeningAsyncServer::new(
                listener,
                service_factory,
                protocol,
            )))
        } else {
            Err(ErrorKind::AttemptToStartServerTwice.into())
        }
    }
}

impl<S, P> Future for AsyncServerStart<S, P>
where
    P: ServerProto<TcpStream>,
    S: NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>,
{
    type Item = ListeningAsyncServer<S, P>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.service_factory.is_some() {
            self.start_server()
        } else {
            Err(ErrorKind::AttemptToStartServerTwice.into())
        }
    }
}
