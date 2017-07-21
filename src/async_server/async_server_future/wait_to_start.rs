use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::{Poll, Sink, Stream};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Handle;
use tokio_proto::pipeline::ServerProto;
use tokio_service::NewService;

use super::state::State;
use super::wait_for_parameters::WaitForParameters;
use super::super::errors::Error;

pub struct WaitToStart<S, P> {
    address: SocketAddr,
    service_factory: S,
    protocol: Arc<Mutex<P>>,
    handle: Handle,
}


impl<S, P> WaitToStart<S, P>
where
    P: ServerProto<TcpStream>,
    S: Clone + NewService<Request = P::Request, Response = P::Response>,
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
            service_factory,
            protocol,
            handle,
        }
    }

    pub fn advance(self) -> (Poll<(), Error>, State<S, P>) {
        let bind_result = TcpListener::bind(&self.address, &self.handle);

        match bind_result {
            Ok(listener) => self.create_server_parameters(listener),
            Err(error) => (Err(error.into()), self.same_state()),
        }
    }

    fn create_server_parameters(
        self,
        listener: TcpListener,
    ) -> (Poll<(), Error>, State<S, P>) {
        let service_factory = self.service_factory.clone();
        let protocol = self.protocol.clone();

        WaitForParameters::advance_with(listener, service_factory, protocol)
    }

    fn same_state(self) -> State<S, P> {
        State::WaitingToStart(self)
    }
}
