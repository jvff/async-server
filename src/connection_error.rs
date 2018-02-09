use std::io;

#[derive(Debug, Fail)]
pub enum ConnectionError {
    #[fail(display = "failed to receive a connection")]
    FailedToReceiveConnection(#[cause] io::Error),

    #[fail(display = "no connections were received")]
    NoConnectionsReceived,
}
