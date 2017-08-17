use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::Future;
use futures::future::Flatten;
use tokio_core::net::TcpStream;
use tokio_core::reactor::{Core, Handle};
use tokio_io::codec::{Decoder, Encoder};
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::errors::{Error, Result};
use super::finite_service::FiniteService;
use super::start_server::StartServer;

pub struct AsyncServer<S, P> {
    address: SocketAddr,
    service_factory: S,
    protocol: Arc<Mutex<P>>,
}

impl<S, P> AsyncServer<S, P>
where
    S: NewService,
    S::Instance: FiniteService,
    S::Request: 'static,
    S::Response: 'static,
    P: Decoder<Item = S::Request>
        + Encoder<Item = S::Response>
        + ServerProto<TcpStream, Request = S::Request, Response = S::Response>,
    Error: From<<P as Decoder>::Error>
        + From<<P as Encoder>::Error>
        + From<<P as ServerProto<TcpStream>>::Error>
        + From<S::Error>,
{
    pub fn new(address: SocketAddr, service_factory: S, protocol: P) -> Self {
        Self {
            address,
            service_factory,
            protocol: Arc::new(Mutex::new(protocol)),
        }
    }

    pub fn serve(self) -> Result<()> {
        let mut reactor = Core::new()?;
        let handle = reactor.handle();
        let server = self.serve_with_handle(handle);

        reactor.run(server)
    }

    pub fn serve_with_handle(
        self,
        handle: Handle,
    ) -> Flatten<Flatten<StartServer<S, P>>> {
        self.start(handle).flatten().flatten()
    }

    pub fn start(self, handle: Handle) -> StartServer<S, P> {
        let address = self.address;
        let protocol = self.protocol;
        let service_factory = self.service_factory;

        StartServer::new(address, service_factory, protocol, handle)
    }
}
