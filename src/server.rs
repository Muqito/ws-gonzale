use {
    crate::AsyncResult,
    async_net::{Incoming, TcpListener},
};
/// A [`TcpListener`] handling incoming [`TcpStream`](`async_net::TcpStream`)
pub struct Server {
    connection: TcpListener,
}
impl<'a> Server {
    /// Opens up a [`TcpListener`] waiting for incoming connections on a given address
    pub async fn new(addr: &str) -> AsyncResult<Server> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Server {
            connection: listener,
        })
    }
    /// Will basically poll-next on an incoming [`TcpStream`]
    pub fn incoming(&self) -> Incoming<'_> {
        self.connection.incoming()
    }
}
