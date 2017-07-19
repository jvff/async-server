use std::collections::VecDeque;
use std::io;
use std::mem;
use std::net::{AddrParseError, SocketAddr};

use futures::future;
use futures::{Async, AsyncSink, Future, IntoFuture, Poll, Sink, StartSend,
              Stream};
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

        ActiveStatusHasNoPollEquivalent {
            description("active server status means processing hasn't finished")
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

    pub fn serve(&mut self) -> Result<()> {
        let mut reactor = Core::new()?;
        let handle = reactor.handle();
        let server = self.serve_with_handle(handle);

        reactor.run(server)
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

#[derive(Debug)]
enum Status {
    Active,
    Finished,
    WouldBlock,
    Error(Error),
}

impl Status {
    fn is_active(&self) -> bool {
        match *self {
            Status::Active => true,
            _ => false
        }
    }

    fn is_more_severe_than(&self, other: &Status) -> bool {
        match (self, other) {
            (_, &Status::Error(_)) => false,
            (&Status::Error(_), _) => true,
            (_, &Status::WouldBlock) => false,
            (&Status::WouldBlock, _) => true,
            (_, &Status::Finished) => false,
            _ => true
        }
    }

    fn update<T: Into<Status>>(&mut self, status_update: T) {
        let status_update = status_update.into();

        if status_update.is_more_severe_than(self) {
            *self = status_update;
        }
    }
}

impl<T, E> From<Poll<T, E>> for Status
where
    E: Into<Error>,
{
    fn from(poll: Poll<T, E>) -> Status {
        match poll {
            Ok(Async::Ready(_)) => Status::Active,
            Ok(Async::NotReady) => Status::WouldBlock,
            Err(error) => Status::Error(error.into()),
        }
    }
}

impl<T, E> From<StartSend<T, E>> for Status
where
    E: Into<Error>,
{
    fn from(start_send: StartSend<T, E>) -> Status {
        match start_send {
            Ok(AsyncSink::Ready) => Status::Active,
            Ok(AsyncSink::NotReady(_)) => Status::WouldBlock,
            Err(error) => Status::Error(error.into()),
        }
    }
}

impl Into<Poll<(), Error>> for Status {
    fn into(self) -> Poll<(), Error> {
        match self {
            Status::Finished => Ok(Async::Ready(())),
            Status::WouldBlock => Ok(Async::NotReady),
            Status::Error(error) => Err(error),
            Status::Active =>
                Err(ErrorKind::ActiveStatusHasNoPollEquivalent.into()),
        }
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
    status: Status,
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
            status: Status::Active,
        }
    }

    fn try_to_get_new_request(&mut self) -> &mut Self {
        if self.status.is_active() {
            let new_request = self.connection.poll();

            if let Ok(Async::Ready(Some(request))) = new_request {
                self.live_requests.push(self.service.call(request));
            } else {
                self.status.update(new_request);
            }
        }

        self
    }

    fn try_to_get_new_response(&mut self) -> &mut Self {
        if self.status.is_active() {
            let maybe_response = self.live_requests.poll();

            if let Ok(Async::Ready(Some(response))) = maybe_response {
                self.live_responses.push_back(response);
            } else {
                self.status.update(maybe_response);
            }
        }

        self
    }

    fn try_to_send_responses(&mut self) -> &mut Self {
        if self.status.is_active() {
            while let Some(response) = self.live_responses.pop_front() {
                match self.connection.start_send(response) {
                    Ok(AsyncSink::Ready) => (),
                    Ok(AsyncSink::NotReady(response)) => {
                        let status_update: Poll<(), Error> =
                            Ok(Async::NotReady);

                        self.live_responses.push_front(response);
                        self.status.update(status_update);
                    }
                    error => self.status.update(error),
                };
            }
        }

        self
    }

    fn try_to_flush_responses(&mut self) -> &mut Self {
        if self.status.is_active() {
            self.status.update(self.connection.poll_complete());
        }

        self
    }

    fn poll_status(&mut self) -> Poll<(), Error> {
        let resulting_status = mem::replace(&mut self.status, Status::Active);

        resulting_status.into()
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
        while self.status.is_active() {
            self.try_to_get_new_request()
                .try_to_get_new_response()
                .try_to_send_responses()
                .try_to_flush_responses();
        }

        self.poll_status()
    }
}
