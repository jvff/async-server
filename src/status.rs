use std::cmp::{Ordering, PartialOrd};

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
        *self > *other
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

impl PartialEq for Status {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (&Status::Error(_), &Status::Error(_)) => false,
            (&Status::WouldBlock, &Status::WouldBlock) => true,
            (&Status::Finished, &Status::Finished) => true,
            (&Status::Active, &Status::Active) => true,
            _ => false,
        }
    }
}

impl PartialOrd for Status {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let ordering = if self.eq(other) {
            Ordering::Equal
        } else {
            match (self, other) {
                (&Status::Error(_), _) => Ordering::Greater,
                (_, &Status::Error(_)) => Ordering::Less,
                (&Status::WouldBlock, _) => Ordering::Greater,
                (_, &Status::WouldBlock) => Ordering::Less,
                (&Status::Finished, _) => Ordering::Greater,
                _ => Ordering::Less,
            }
        };

        Some(ordering)
    }
}
