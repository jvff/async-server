use super::super::connection_error::ConnectionError;

#[derive(Debug, Fail)]
pub enum BindConnectionError<P> {
    #[fail(display = "bind connection failed")]
    BindError(#[cause] P),

    #[fail(display = "no connection to bind")]
    NoConnectionToBind(#[cause] ConnectionError),

    #[fail(display = "an other thread panicked with the protocol locked")]
    ProtocolLockError,
}
