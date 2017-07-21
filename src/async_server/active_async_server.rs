use std::collections::VecDeque;
use std::mem;

use futures::{Async, AsyncSink, Future, Poll, Sink, Stream};
use futures::stream::FuturesUnordered;
use tokio_service::Service;

use super::errors::Error;
use super::status::Status;

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
    pub fn new(connection: T, service: S) -> Self {
        Self {
            connection,
            service,
            live_requests: FuturesUnordered::new(),
            live_responses: VecDeque::new(),
            status: Status::Active,
        }
    }

    pub fn from_tuple((connection, service): (T, S)) -> Self {
        Self::new(connection, service)
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
                        let status_update: Poll<
                            (),
                            Error,
                        > = Ok(Async::NotReady);

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
