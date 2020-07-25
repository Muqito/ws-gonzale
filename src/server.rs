use {
    crate::AsyncResult,
    async_net::{Incoming, TcpListener},
};
pub struct Server {
    connection: TcpListener,
}
impl<'a> Server {
    pub async fn new(addr: &str) -> AsyncResult<Server> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Server {
            connection: listener,
        })
    }
    pub fn incoming(&self) -> Incoming<'_> {
        self.connection.incoming()
    }
}
