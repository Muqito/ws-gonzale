use {
    crate::lib::server::{ServerData, ServerMessage},
    std::sync::atomic::{AtomicUsize, Ordering},
    ws_gonzale::{
        async_channel::Sender,
        async_std::{sync::Arc, task, task::JoinHandle},
        async_trait::async_trait,
        futures::StreamExt,
        Channels, Message, Server, WsClientHook, WsConnection,
    },
};
static ID: AtomicUsize = AtomicUsize::new(1);

struct ConnectionEvents {
    id: u32,
    server_sender: Sender<ServerMessage>,
    channels: Option<Channels>,
}
impl ConnectionEvents {
    pub fn new(server_sender: Sender<ServerMessage>) -> ConnectionEvents {
        Self {
            id: ID.fetch_add(1, Ordering::SeqCst) as u32,
            channels: None,
            server_sender,
        }
    }
    fn get_id(&self) -> u32 {
        self.id
    }
}
#[async_trait]
impl WsClientHook for ConnectionEvents {
    async fn after_handshake(&mut self) -> Result<(), ()> {
        if let Some(channels) = self.channels.take() {
            let _ = self
                .server_sender
                .send(ServerMessage::ClientJoined((self.get_id(), channels)))
                .await;
        }
        Ok(())
    }

    async fn after_drop(&self) -> Result<(), ()> {
        let _ = self
            .server_sender
            .send(ServerMessage::ClientDisconnected(self.get_id()))
            .await;
        Ok(())
    }

    async fn on_message(&self, message: &Message) -> Result<(), ()> {
        let _ = self
            .server_sender
            .send(ServerMessage::ClientMessage(message.clone().to_owned()))
            .await;
        Ok(())
    }

    fn set_channels(&mut self, channels: Channels) {
        self.channels = Some(channels);
    }
}
pub fn connections(server_data: Arc<ServerData>) -> JoinHandle<Result<(), std::io::Error>> {
    task::spawn(async move {
        // TODO: Extract this from a Config struct that's built with .dotenv or something
        let server = Server::new("127.0.0.1:8080").await?;
        let mut incoming = server.incoming();
        while let Some(Ok(connection)) = incoming.next().await {
            let server_sender = server_data.get_channel_sender();
            task::spawn(async move {
                let mut ws_connection =
                    WsConnection::upgrade(connection, ConnectionEvents::new(server_sender.clone()))
                        .await?;
                // You could loop over messages here like this instead of using ConnectionEvents
                // My thinking is that if I someday wanted to branch out ws-common to a separate crate;
                // WsClientHook couldn't just be implemented on WsConnection since they're both coming from the library.
                /*                loop {
                    if let message = ws_connection.incoming_message().await? {
                        dbg!(message);
                    }
                }*/
                loop {
                    if let Message::Close = ws_connection.incoming_message().await? {
                        println!("Manually closed");
                        break;
                    }
                }
                // Cannot refer type, see: https://rust-lang.github.io/async-book/07_workarounds/03_err_in_async_blocks.html
                Ok::<(), std::io::Error>(())
            });
        }

        Ok::<(), std::io::Error>(())
    })
}
