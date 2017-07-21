use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::future;
use futures::{Future, IntoFuture};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Core, Handle};
use tokio_io::codec::{Decoder, Encoder};
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::active_async_server::ActiveAsyncServer;
use super::bound_connection_future::BoundConnectionFuture;
use super::errors::{Error, NormalizeError, Result};

pub struct AsyncServer<S, P> {
    address: SocketAddr,
    service_factory: S,
    protocol: Arc<Mutex<P>>,
}

pub type ServerFuture = Box<Future<Item = (), Error = Error>>;

impl<S, P> AsyncServer<S, P>
where
    S: NewService,
    S::Instance: 'static,
    S::Request: 'static,
    S::Response: 'static,
    P: Decoder<Item = S::Request>
        + Encoder<Item = S::Response>
        + ServerProto<TcpStream, Request = S::Request, Response = S::Response>,
    Error: From<<P as Decoder>::Error>
        + From<<P as Encoder>::Error>
        + From<S::Error>,
{
    pub fn new(address: SocketAddr, service_factory: S, protocol: P) -> Self {
        Self {
            address,
            service_factory,
            protocol: Arc::new(Mutex::new(protocol)),
        }
    }

    pub fn serve(&mut self) -> Result<()> {
        let mut reactor = Core::new()?;
        let handle = reactor.handle();
        let server = self.serve_with_handle(handle);

        reactor.run(server)
    }

    pub fn serve_with_handle(&mut self, handle: Handle) -> ServerFuture {
        match TcpListener::bind(&self.address, &handle) {
            Ok(listener) => self.serve_on_listener(listener),
            Err(error) => Box::new(future::err(error.into())),
        }
    }

    fn serve_on_listener(&mut self, listener: TcpListener) -> ServerFuture {
        let protocol = self.protocol.clone();
        let service = self.service_factory.new_service().into_future();
        let connection = BoundConnectionFuture::from(listener, protocol);

        let server = connection
            .join(service.normalize_error())
            .map(ActiveAsyncServer::from_tuple)
            .flatten();

        Box::new(server)
    }
}
