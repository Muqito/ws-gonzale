use async_channel::{Receiver, Sender};

pub mod connection;
pub mod dataframe;
pub mod handshake;
pub mod message;
pub mod server;

pub use self::connection::*;
pub use self::dataframe::*;
pub use self::handshake::*;
pub use self::message::*;
pub use self::server::*;

pub use async_channel;
pub use async_net;
pub use async_std;
pub use async_trait;
pub use futures;

pub type AsyncResult<T> = std::io::Result<T>;
pub type Channel<T> = (Sender<T>, Receiver<T>);
