use {
    crate::lib::server::{ServerData, ServerMessage},
    std::sync::atomic::{AtomicUsize, Ordering},
    ws_gonzale::{
        async_channel::Sender,
        async_std::{sync::Arc, task, task::JoinHandle},
        async_trait::async_trait,
        futures::StreamExt,
        Channels, Headers, Message, Server, WsClientHook, WsConnection, WsEvents,
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
        while let Some(Ok(mut connection)) = incoming.next().await {
            let server_sender = server_data.get_channel_sender();
            task::spawn(async move {
                // We extracted out TcpStream read from WsConnection so we can be more flexible in the implementation
                let default_str = String::new();
                let headers = Headers::read_from_stream(&mut connection).await?;
                let key = headers.get("Sec-WebSocket-Key").unwrap_or(&default_str);

                // Upgrade to WS connection because the run cycle and reading dataframes assumes a WSConnection
                let ws_connection = WsConnection::upgrade(connection, key).await?;

                // Run cycle
                let ws_events =
                    WsEvents::new(ws_connection, ConnectionEvents::new(server_sender)).await?;
                let _ = ws_events.run().await?;
                Ok::<_, std::io::Error>(())
            });
        }

        Ok::<(), std::io::Error>(())
    })
}
