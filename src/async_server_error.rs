use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::io;
use std::sync::Mutex;

use super::errors::Error as OldError;
use super::errors::ErrorKind as OldErrorKind;

#[derive(Debug, Fail)]
pub enum AsyncServerError<S> {
    #[fail(display = "StartServer can't shutdown server after it started")]
    IncorrectShutdownInStartServer,

    #[fail(display = "AsyncServer was shut down")]
    ServerWasShutDown,

    #[fail(display = "service creation error")]
    ServiceCreationError(#[cause] io::Error),

    #[fail(display = "service error")]
    ServiceShutdownError(#[cause] S),

    #[fail(display = "AsyncServer is shutting down")]
    ShuttingDown,

    #[fail(display = "old format error")]
    OldError(#[cause] OldErrorWrapper),
}

impl<S> From<OldError> for AsyncServerError<S> {
    fn from(error: OldError) -> Self {
        let wrapped_error = OldErrorWrapper(Mutex::new(error));

        AsyncServerError::OldError(wrapped_error)
    }
}

impl<S> From<OldErrorKind> for AsyncServerError<S> {
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
