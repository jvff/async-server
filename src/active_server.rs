use std::collections::VecDeque;
use std::mem;

use futures::{Async, AsyncSink, Future, Poll, Sink, Stream};
use futures::stream::FuturesUnordered;

use super::async_server_error::AsyncServerError;
use super::errors::Error;
use super::finite_service::FiniteService;
use super::status::Status;

pub struct ActiveServer<S, T>
where
    S: FiniteService,
{
    connection: T,
    service: S,
    live_requests: FuturesUnordered<S::Future>,
    live_responses: VecDeque<S::Response>,
    status: Status,
}

impl<S, T> ActiveServer<S, T>
where
    S: FiniteService,
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

    pub fn shutdown(&mut self) -> Poll<(), Error> {
        match self.service.force_stop() {
            Ok(()) => Ok(Async::Ready(())),
            Err(error) => Err(error.into()),
        }
    }

    fn try_to_get_new_request(&mut self) -> &mut Self {
        if self.status.is_running() {
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
        if self.status.is_running() {
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
        if self.status.is_running() {
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
        if self.status.is_running() {
            self.status.update(self.connection.poll_complete());
        }

        self
    }

    fn check_if_finished(&mut self) {
        if self.status.is_running() {
            let no_pending_requests = self.live_requests.is_empty();
            let no_pending_responses = self.live_responses.is_empty();

            if no_pending_requests && no_pending_responses {
                let service_status = match self.service.has_finished() {
                    Ok(true) => Status::Finished,
                    Ok(false) => Status::Active,
                    Err(error) => Status::Error(error.into()),
                };

                self.status.update(service_status);
            }
        }
    }

    fn poll_status(&mut self) -> Poll<(), AsyncServerError> {
        let resulting_status = mem::replace(&mut self.status, Status::Active);

        resulting_status.into()
    }
}

impl<S, T> Future for ActiveServer<S, T>
where
    S: FiniteService,
    T: Sink<SinkItem = S::Response> + Stream<Item = S::Request>,
    Error: From<S::Error> + From<T::SinkError> + From<T::Error>,
{
    type Item = ();
    type Error = AsyncServerError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        while self.status.is_active() {
            self.try_to_get_new_request()
                .try_to_get_new_response()
                .try_to_send_responses()
                .try_to_flush_responses()
                .check_if_finished();
        }

        self.poll_status()
    }
}
