use std::io;
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

        ActiveStatusHasNoPollEquivalent {
            description("active server status means processing hasn't finished")
        }
    }
}
