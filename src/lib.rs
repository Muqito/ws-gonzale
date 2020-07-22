use async_channel::{Sender, Receiver};

pub mod server;
pub mod message;
pub mod handshake;
pub mod dataframe;
pub mod connection;

pub use self::server::*;
pub use self::message::*;
pub use self::handshake::*;
pub use self::dataframe::*;
pub use self::connection::*;

pub use async_net;
pub use async_std;
pub use async_channel;
pub use async_trait;
pub use futures;

pub type AsyncResult<T> = std::io::Result<T>;
pub type Channel<T> = (Sender<T>, Receiver<T>);