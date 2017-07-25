use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::{Async, Future, Poll, Sink, Stream};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Handle;
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::errors::Error;
use super::async_server_future::AsyncServerFuture;

pub struct AsyncServerStart<S, P> {
    address: SocketAddr,
    service_factory: S,
    protocol: Arc<Mutex<P>>,
    handle: Handle,
}

impl<S, P> AsyncServerStart<S, P>
where
    S: NewService,
    P: ServerProto<TcpStream>,
{
    pub fn new(
        address: SocketAddr,
        service_factory: S,
        protocol: Arc<Mutex<P>>,
        handle: Handle,
    ) -> Self {
        Self {
            address,
            service_factory,
            protocol,
            handle,
        }
    }
}

impl<S, P> Future for AsyncServerStart<S, P>
where
    P: ServerProto<TcpStream>,
    S: Clone + NewService<Request = P::Request, Response = P::Response>,
    Error: From<S::Error>
        + From<<P::Transport as Stream>::Error>
        + From<<P::Transport as Sink>::SinkError>,
{
    type Item = AsyncServerFuture<S, P>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let service_factory = self.service_factory.clone();
        let protocol = self.protocol.clone();

        let listener = TcpListener::bind(&self.address, &self.handle)?;
        let future =
            AsyncServerFuture::new(listener, service_factory, protocol);

        Ok(Async::Ready(future))
    }
}