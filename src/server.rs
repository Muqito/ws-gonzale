use {
    crate::AsyncResult,
    async_net::{Incoming, TcpListener},
    async_std::net::SocketAddr,
};

/// A [`TcpListener`] handling incoming [`TcpStream`](`async_net::TcpStream`)
pub struct Server {
    connection: TcpListener,
}
impl<'a> Server {
    /// Opens up a [`TcpListener`] waiting for incoming connections on a given address
    pub async fn new(socket_addr: SocketAddr) -> AsyncResult<Server> {
        let listener = TcpListener::bind(socket_addr).await?;
        Ok(Server {
            connection: listener,
        })
    }
    /// Will basically poll-next on an incoming [`TcpStream`](`async_net::TcpStream`)
    pub fn incoming(&self) -> Incoming<'_> {
        self.connection.incoming()
    }
}
