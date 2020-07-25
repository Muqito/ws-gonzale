use {
    crate::{dataframe, dataframe::get_buffer, handshake, message::Message, AsyncResult, Channel},
    async_channel::Sender,
    async_net::TcpStream,
    async_std::task,
    async_trait::async_trait,
    futures::{AsyncReadExt, AsyncWriteExt},
};

pub type Channels = (Sender<Vec<u8>>, TcpStream);

#[async_trait]
pub trait WsClientHook {
    /// Once the user has been upgraded from a regular HTTP GET request to a WS connection that's kept open.
    async fn after_handshake(&mut self) -> Result<(), ()>;
    /// Once the connection has dropped, this is async so we can wait for this because drop doesn't have an async implementation yet/ever?
    async fn after_drop(&self) -> Result<(), ()>;
    /// When we've interpreted a complete WS frame packet
    async fn on_message(&self, message: &Message) -> Result<(), ()>;
    /// This our multi producer / multi consumer channel. (Could be done with a mpsc channel as well since we only ever use this once in the code?)
    fn set_channels(&mut self, ws_writer: Channels);
}
/// Our WSConnection after it's been upgraded from a TCPStream
pub struct WsConnection {
    /// Our TcpStream from `async-net`
    tcp_stream: TcpStream,
    /// Our multi producer / multi consumer channel channels we are creating upon creating the connection
    channel: Channel<Vec<u8>>,
    /// Client hooks; we could do this in the life cycle; but I wanted the library to be as easily implemented as possible for end users.
    /// So we'll have to deal with wrapping this behind a pointer (Boxing it here) since we don't know the size of the struct developers will implement WsClientHook on.
    client_hook: Box<dyn WsClientHook + Send + Sync>,
}

impl WsConnection {
    /// Upgrades the TcpStream to a WsConnection that's basically a handshake between a client and server
    /// and the connection is kept open.
    pub async fn upgrade(
        tcp_stream: TcpStream,
        client_hook: impl WsClientHook + Send + Sync + 'static,
    ) -> AsyncResult<WsConnection> {
        let mut connection = WsConnection {
            tcp_stream,
            channel: async_channel::unbounded(),
            client_hook: Box::new(client_hook),
        };
        // Before returning the WsConnection; make sure the handshake is done.
        connection
            .handshake()
            .await
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::Interrupted))?;

        Ok(connection)
    }
    /// Read incoming data packets from tcp stream
    pub async fn incoming_message(&mut self) -> AsyncResult<Message> {
        let mut buffer: [u8; 2] = [0; 2];

        // Do a peek-ahead so we can utilize the read_exact of the full payload and then use From<&[u8]> for Dataframe
        match self.tcp_stream.peek(&mut buffer).await {
            // Connection was aborted
            Ok(s) if s == 0 => {
                return Err(std::io::Error::from(std::io::ErrorKind::ConnectionAborted))
            }
            // fin(126) + opcode for close(8), see rfc protocol.. just ignore the reason.
            Ok(_) if buffer.len() > 0 && buffer[0] == 136 => return Ok(Message::Close),
            Ok(_) => {}
            // Upon error, return early
            Err(err) => return Err(err),
        };

        // Just peek ahead for the largest data package 127 in size. That's 2 (two first frames including fin, rsv1-3, mask and payload_length) + 8 (u64 size in bytes) + 4 (masking_key) = 14
        let mut peeked_buff: [u8; 14] = [0; 14];
        self.tcp_stream.peek(&mut peeked_buff).await?;

        let dataframe = dataframe::DataframeBuilder::new(peeked_buff.to_vec()).unwrap();
        let mut payload: Vec<u8> = vec![0; dataframe.get_full_frame_length() as usize];

        self.tcp_stream.read_exact(&mut payload).await?;

        let dataframe = dataframe::DataframeBuilder::new(payload)?;
        // Unwrap since we already have a .or() in place. If no message or `Message::Unknown` just return `Message::Unknown`
        let message = dataframe.get_message().or(Some(Message::Unknown)).unwrap();

        let _ = self.client_hook.on_message(&message).await;

        Ok(message)
    }
    async fn handshake(&mut self) -> Result<(), std::io::Error> {
        let _ = handshake::handshake(&mut self.tcp_stream).await;
        // After handshake, notify WsClientHook struct that a handshake has been successful.
        // Also pass a long a multi-producer `Sender<Vec<u8>>` so we can receive data from outside this lib.
        self.setup_listeners();
        // Notify the client_hook that everything is good to go so they can interact with this WsConnection.
        let _ = self.client_hook.after_handshake().await;
        Ok(())
    }
    /// Clones the Sender channel and returns it. This is so we can have multiple places where we can send to this channel if desired.
    /// Setup a reader of the multi producer and write to the underlying tcp_stream of our guest client.
    fn setup_listeners(&mut self) {
        // Send the WsWriter to this stream to the client hook
        self.client_hook
            .set_channels((self.channel.0.clone(), self.tcp_stream.clone()));

        // Clone this because we are moving it into a new future which could be on another thread.
        let reader = self.channel.1.clone();
        // Same idea here; we need to clone this so we can keep reading from tcp_stream in incoming_message
        let mut tcp_stream = self.tcp_stream.clone();
        task::spawn(async move {
            // A nice welcome message once everything is setup, we are making sure this is the first thing the users see because of the await.
            let _ = tcp_stream
                .write_all(&get_buffer(Message::Text("Welcome message!".to_string())))
                .await;
            while let Ok(buffer) = reader.recv().await {
                let _ = tcp_stream.write_all(&buffer).await;
            }
        });
    }
}

/// This Drop method around WsConnection is pretty neat. Makes sure we are notifying the developer created struct implemented WsClientHook that the
/// WsConnection has been dropped
impl Drop for WsConnection {
    fn drop(&mut self) {
        // Block this thread until notified since Drop doesn't support async
        let _ = async_std::task::block_on(self.client_hook.after_drop());
    }
}
