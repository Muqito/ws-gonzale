pub mod connection;
pub mod dataframe;
pub mod handshake;
pub mod message;
pub mod server;
pub mod channel;

pub use self::connection::*;
pub use self::dataframe::*;
pub use self::handshake::*;
pub use self::message::*;
pub use self::server::*;
pub use self::channel::*;

pub use async_net;
pub use async_std;
pub use async_trait;
pub use futures;

pub use channel::{Receiver, Sender};

pub type Channel<T> = (Sender<T>, Receiver<T>);
pub type AsyncResult<T> = Result<T, std::io::Error>;
pub type WsGonzaleResult<T> = Result<T, WsGonzaleError>;

#[derive(Debug, PartialEq)]
pub enum WsGonzaleError {
    InvalidPayload,
    ConnectionClosed,
    Unknown,
}
impl From<std::io::Error> for WsGonzaleError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::AddrInUse
            | std::io::ErrorKind::AddrNotAvailable
            | std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::NotConnected => WsGonzaleError::ConnectionClosed,
            _ => WsGonzaleError::Unknown,
        }
    }
}
impl From<WsGonzaleError> for std::io::Error {
    fn from(error: WsGonzaleError) -> Self {
        let error_kind = match error {
            WsGonzaleError::ConnectionClosed => std::io::ErrorKind::ConnectionAborted,
            WsGonzaleError::InvalidPayload => std::io::ErrorKind::InvalidData,
            _ => std::io::ErrorKind::Other,
        };
        std::io::Error::from(error_kind)
    }
}
