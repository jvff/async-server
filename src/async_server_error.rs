use std::io;

use super::bound_connection_future::BindConnectionError;

#[derive(Debug, Fail)]
pub enum AsyncServerError<S, P> {
    #[fail(display = "can't start server using the same future more than once")]
    AttemptToStartServerTwice,

    #[fail(display = "failed to bind to socket")]
    BindSocketError(#[cause] io::Error),

    #[fail(display = "failed to bind connection into protocol transport")]
    BindError(#[cause] BindConnectionError<P>),

    #[fail(display = "failed to flush responses in protocol transport")]
    FlushResponsesError(#[cause] P),

    #[fail(display = "ListeningServer can't shutdown server after a connection is made")]
    IncorrectShutdownInListeningServer,

    #[fail(display = "StartServer can't shutdown server after it started")]
    IncorrectShutdownInStartServer,

    #[fail(display = "ListeningServer can't be polled more than once")]
    ListenedTwice,

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
}
