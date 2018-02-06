use std::collections::VecDeque;
use std::io;
use std::net::{AddrParseError, SocketAddr};

use futures::future;
use futures::{Async, AsyncSink, Future, IntoFuture, Poll, Sink, Stream};
use futures::stream::FuturesUnordered;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Core, Handle};
use tokio_io::codec::{Decoder, Encoder};
use tokio_proto::pipeline::ServerProto;
use tokio_service::{NewService, Service};

error_chain! {
    foreign_links {
        Io(io::Error);
        InvalidAddressToBindTo(AddrParseError);
    }

    errors {
        FailedToReceiveConnection {
            description("failed to receive a connection")
        }
    }
}

pub struct AsyncServer<S, P> {
    address: SocketAddr,
    service_factory: S,
    protocol: P,
}

pub type ServerFuture = Box<Future<Item = (), Error = Error>>;

impl<S, P> AsyncServer<S, P>
where
    S: NewService,
    S::Instance: 'static + Send,
    S::Request: 'static,
    S::Response: 'static + Send,
    P: Clone
        + Decoder<Item = S::Request>
        + Encoder<Item = S::Response>
        + Send
        + ServerProto<TcpStream, Request = S::Request, Response = S::Response>,
    Error: From<<P as Decoder>::Error>
        + From<<P as Encoder>::Error>
        + From<S::Error>,
{
    pub fn new(address: SocketAddr, service_factory: S, protocol: P) -> Self {
        Self {
            address,
            service_factory,
            protocol,
        }
    }

    pub fn serve(&mut self) -> ServerFuture {
        match Core::new() {
            Ok(reactor) => self.serve_with_handle(reactor.handle()),
            Err(error) => Box::new(future::err(error.into()))
        }
    }

    pub fn serve_with_handle(&mut self, handle: Handle) -> ServerFuture {
        match TcpListener::bind(&self.address, &handle) {
            Ok(listener) => self.serve_on_listener(listener),
            Err(error) => Box::new(future::err(error.into()))
        }
    }

    fn serve_on_listener(&mut self, listener: TcpListener) -> ServerFuture {
        let connections = listener.incoming();
        let single_connection = connections.take(1)
            .into_future()
            .map_err::<_, Error>(|(error, _)| error.into())
            .and_then(|(maybe_connection, _)| {
                let no_connections = ErrorKind::FailedToReceiveConnection;
                let no_connections: Error = no_connections.into();

                maybe_connection.ok_or(no_connections)
            });

        let service = self.service_factory.new_service().into_future();
        let protocol = self.protocol.clone();
        let server = single_connection.and_then(move |(socket, _)| {
            protocol.bind_transport(socket)
                .into_future()
                .join(service)
                .map_err(|error| error.into())
                .and_then(|(connection, service)| {
                    ActiveAsyncServer::new(connection, service)
                })
        });

        Box::new(server)
    }
}

pub struct ActiveAsyncServer<S, T>
where
    S: Service,
{
    connection: T,
    service: S,
    live_requests: FuturesUnordered<S::Future>,
    live_responses: VecDeque<S::Response>,
}

impl<S, T> ActiveAsyncServer<S, T>
where
    S: Service,
    T: Sink<SinkItem = S::Response> + Stream<Item = S::Request>,
    Error: From<S::Error> + From<T::SinkError> + From<T::Error>,
{
    fn new(connection: T, service: S) -> Self {
        Self {
            connection,
            service,
            live_requests: FuturesUnordered::new(),
            live_responses: VecDeque::new(),
        }
    }

    fn try_to_get_new_request(&mut self) -> Result<()> {
        let new_request = self.connection.poll();

        if let Ok(Async::Ready(Some(request))) = new_request {
            self.live_requests.push(self.service.call(request));
            Ok(())
        } else {
            new_request.and(Ok(())).map_err(|error| error.into())
        }
    }

    fn try_to_get_new_response(&mut self) -> Result<()> {
        let maybe_response = self.live_requests.poll();

        if let Ok(Async::Ready(Some(response))) = maybe_response {
            self.live_responses.push_back(response);
            Ok(())
        } else {
            maybe_response.and(Ok(())).map_err(|error| error.into())
        }
    }

    fn try_to_send_responses(&mut self) -> Poll<(), Error> {
        while let Some(response) = self.live_responses.pop_front() {
            match self.connection.start_send(response)? {
                AsyncSink::Ready => (),
                AsyncSink::NotReady(response) => {
                    self.live_responses.push_front(response);

                    return Ok(Async::NotReady);
                }
            };
        }

        Ok(Async::Ready(()))
    }

    fn try_to_flush_responses(&mut self) -> Poll<(), Error> {
        self.connection.poll_complete().map_err(|error| error.into())
    }
}

impl<S, T> Future for ActiveAsyncServer<S, T>
where
    S: Service,
    T: Sink<SinkItem = S::Response> + Stream<Item = S::Request>,
    Error: From<S::Error> + From<T::SinkError> + From<T::Error>,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.try_to_get_new_request()
            .and_then(|_| self.try_to_get_new_response())
            .and_then(|_| self.try_to_send_responses())
            .and_then(|_| self.try_to_flush_responses())
            .and_then(|status| {
                let pending_requests = !self.live_requests.is_empty();
                let pending_responses = !self.live_responses.is_empty();

                if pending_requests || pending_responses {
                    Ok(Async::NotReady)
                } else {
                    Ok(status)
                }
            })
    }
}
