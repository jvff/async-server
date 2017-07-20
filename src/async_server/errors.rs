use std::{io, result};
use std::net::AddrParseError;

error_chain! {
    foreign_links {
        Io(io::Error);
        InvalidAddressToBindTo(AddrParseError);
    }

    errors {
        FailedToReceiveConnection {
            description("failed to receive a connection")
        }

        FailedToBindConnection {
            description("failed to bind the connection to receive requests")
        }

        ActiveStatusHasNoPollEquivalent {
            description("active server status means processing hasn't finished")
        }
    }
}

pub trait NormalizeError<T> {
    fn normalize_error(self) -> T;
}

impl<T, E> NormalizeError<Result<T>> for result::Result<T, E>
where
    E: Into<Error>,
{
    fn normalize_error(self) -> Result<T> {
        self.map_err(|error| -> Error { error.into() })
    }
}
