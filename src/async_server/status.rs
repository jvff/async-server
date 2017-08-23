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

    pub fn is_running(&self) -> bool {
        match *self {
            Status::Active => true,
            Status::WouldBlock => true,
            _ => false,
        }
    }

    pub fn update<T: Into<Status>>(&mut self, status_update: T) {
        let status_update = status_update.into();

        if status_update.is_more_severe_than(self) {
            *self = status_update;
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

impl<E> Into<Poll<(), E>> for Status
where
    E: From<Error>,
{
    fn into(self) -> Poll<(), E> {
        match self {
            Status::Finished => Ok(Async::Ready(())),
            Status::WouldBlock => Ok(Async::NotReady),
            Status::Error(error) => Err(error.into()).into(),
            Status::Active => {
                let error_kind = ErrorKind::ActiveStatusHasNoPollEquivalent;
                let error: Error = error_kind.into();

                Err(error.into())
            }
        }
    }
}
