use futures::{Async, AsyncSink, Poll, StartSend};

use super::errors::{Error, ErrorKind};

#[derive(Debug)]
pub enum Status {
    Active,
    Finished,
    WouldBlock,
    Error(Error),
}

impl Status {
    pub fn is_active(&self) -> bool {
        match *self {
            Status::Active => true,
            _ => false,
        }
    }

    fn is_more_severe_than(&self, other: &Status) -> bool {
        match (self, other) {
            (_, &Status::Error(_)) => false,
            (&Status::Error(_), _) => true,
            (_, &Status::WouldBlock) => false,
            (&Status::WouldBlock, _) => true,
            (_, &Status::Finished) => false,
            _ => true,
        }
    }

    pub fn update<T: Into<Status>>(&mut self, status_update: T) {
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
            Status::Active => {
                Err(ErrorKind::ActiveStatusHasNoPollEquivalent.into())
            }
        }
    }
}
