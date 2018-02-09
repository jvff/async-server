use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::io;
use std::sync::Mutex;

use super::bound_connection_future::BindConnectionError;
use super::errors::Error as OldError;
use super::errors::ErrorKind as OldErrorKind;

#[derive(Debug, Fail)]
pub enum AsyncServerError<S, P> {
    #[fail(display = "failed to bind connection into protocol transport")]
    BindError(#[cause] BindConnectionError<P>),

    #[fail(display = "failed to flush responses in protocol transport")]
    FlushResponsesError(#[cause] P),

    #[fail(display = "ListeningServer can't shutdown server after a connection is made")]
    IncorrectShutdownInListeningServer,

    #[fail(display = "StartServer can't shutdown server after it started")]
    IncorrectShutdownInStartServer,

    #[fail(display = "failed to get a new request from the protocol transport")]
    NewRequestError(#[cause] P),

    #[fail(display = "failed to get a response from the service")]
    NewResponseError(#[cause] S),

    #[fail(display = "failed to send response through protocol transport")]
    SendResponseError(#[cause] P),

    #[fail(display = "AsyncServer was shut down")]
    ServerWasShutDown,

    #[fail(display = "service creation error")]
    ServiceCreationError(#[cause] io::Error),

    #[fail(display = "service failed when asked if it had finished")]
    ServiceFinishedCheckError(#[cause] S),

    #[fail(display = "service error")]
    ServiceShutdownError(#[cause] S),

    #[fail(display = "AsyncServer is shutting down")]
    ShuttingDown,

    #[fail(display = "old format error")]
    OldError(#[cause] OldErrorWrapper),
}

impl<S, P> From<OldError> for AsyncServerError<S, P> {
    fn from(error: OldError) -> Self {
        let wrapped_error = OldErrorWrapper(Mutex::new(error));

        AsyncServerError::OldError(wrapped_error)
    }
}

impl<S, P> From<OldErrorKind> for AsyncServerError<S, P> {
    fn from(error_kind: OldErrorKind) -> Self {
        OldError::from(error_kind).into()
    }
}

#[derive(Debug)]
pub struct OldErrorWrapper(Mutex<OldError>);

impl Display for OldErrorWrapper {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self.0.lock() {
            Ok(error) => error.fmt(formatter),
            _ => write!(formatter, "failed to display error information"),
        }
    }
}

impl Error for OldErrorWrapper {
    fn description(&self) -> &str {
        &"failed to display error description"
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}
