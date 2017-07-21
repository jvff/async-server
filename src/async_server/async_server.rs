use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio_core::net::TcpStream;
use tokio_core::reactor::{Core, Handle};
use tokio_io::codec::{Decoder, Encoder};
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::async_server_future::AsyncServerFuture;
use super::errors::{Error, Result};

pub struct AsyncServer<S, P> {
    address: SocketAddr,
    service_factory: S,
    protocol: Arc<Mutex<P>>,
}

impl<S, P> AsyncServer<S, P>
where
    S: Clone + NewService,
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

    pub fn serve_with_handle(
        &mut self,
        handle: Handle,
    ) -> AsyncServerFuture<S, P> {
        let address = self.address.clone();
        let protocol = self.protocol.clone();
        let service_factory = self.service_factory.clone();

        AsyncServerFuture::new(address, service_factory, protocol, handle)
    }
}
