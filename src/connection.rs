use {
    crate::{channel, dataframe, handshake, message::Message, Channel, Sender, WsGonzaleResult},
    async_net::TcpStream,
    async_std::task,
    async_trait::async_trait,
    futures::{AsyncReadExt, AsyncWriteExt},
};

/// Channels is sent to the struct implementing [`WsClientHook`] so they can use it to send to the mpmc channel or directly to the [`TcpStream`]
pub type Channels = (Sender<Vec<u8>>, TcpStream);

/// Trait that's used on a struct passed to [`WsConnection`] so we can set [`Channels`] but also listen for events.
#[async_trait]
pub trait WsClientHook {
    /// Once the user has been upgraded from a regular HTTP GET request to a WS connection that's kept open.
    fn after_handshake(&mut self) -> Result<(), ()>;
    /// Once the connection has dropped, this is async so we can wait for this because drop doesn't have an async implementation yet/ever?
    fn after_drop(&self) -> Result<(), ()>;
    /// When we've interpreted a complete WS frame packet
    fn on_message(&self, message: &Message) -> Result<(), ()>;
    /// This our multi producer / multi consumer channel. (Could be done with a mpsc channel as well since we only ever use this once in the code?)
    fn set_channels(&mut self, ws_writer: Channels);
}
#[derive(Clone)]
/// Our WSConnection after it's been upgraded from a TCPStream
pub struct WsConnection(TcpStream);
impl WsConnection {
    pub fn get_tcp_stream(&self) -> TcpStream {
        self.0.clone()
    }
}
/// Handles WebSocket incoming data frames and sends back to [`WsClientHook`] methods.
pub struct WsEvents {
    ws_connection: WsConnection,
    /// Client hooks; we could do this in the life cycle; but I wanted the library to be as easily implemented as possible for end users.
    /// So we'll have to deal with wrapping this behind a pointer (Boxing it here) since we don't know the size of the struct developers will implement WsClientHook on.
    client_hook: Box<dyn WsClientHook + Send + Sync>,
}
impl WsEvents {
    /// Upgrades the TcpStream to a WsConnection that's basically a handshake between a client and server
    /// and the connection is kept open.
    pub async fn new(
        ws_connection: WsConnection,
        client_hook: impl WsClientHook + Send + Sync + 'static,
    ) -> WsGonzaleResult<WsEvents> {
        let mut ws_events = WsEvents {
            ws_connection,
            client_hook: Box::new(client_hook),
        };

        let _ = ws_events.setup_listeners().await;

        Ok(ws_events)
    }
    /// Clones the Sender channel and returns it. This is so we can have multiple places where we can send to this channel if desired.
    /// Setup a reader of the multi producer and write to the underlying tcp_stream of our guest client.
    async fn setup_listeners(&mut self) -> WsGonzaleResult<()> {
        let (tx, mut rx) = channel::unbounded();
        // Send the WsWriter to this stream to the client hook
        self.client_hook
            .set_channels((tx.clone(), self.ws_connection.get_tcp_stream()));

        // Same idea here; we need to clone this so we can keep reading from tcp_stream in incoming_message
        let mut tcp_stream_writer = self.ws_connection.get_tcp_stream();

        task::spawn(async move {
            while let Some(buffer) = rx.recv() {
                let _ = tcp_stream_writer.write_all(&buffer).await;
            }
            Ok::<(), std::io::Error>(())
        });

        let _ = self.client_hook.after_handshake();
        Ok(())
    }
    /// This is the run which handles the WsEvents lifecycle.
    /// Here we take full ownership because when we are done; we should drop the connection.
    pub async fn run(mut self) -> WsGonzaleResult<()> {
        while let Ok(message) = self.ws_connection.incoming_message().await {
            if Message::Close == message {
                break;
            } else {
                // pass events to client hook
                let _ = self.client_hook.on_message(&message);
            }
        }
        Ok(())
    }
}

impl WsConnection {
    /// Upgrades the TcpStream to a WsConnection that's basically a handshake between a client and server
    /// and the connection is kept open.
    pub async fn upgrade(tcp_stream: TcpStream, accept_key: &str) -> WsGonzaleResult<WsConnection> {
        let mut connection = WsConnection(tcp_stream);
        // Before returning the WsConnection; make sure the handshake is done.
        connection
            .handshake(accept_key)
            .await
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::Interrupted))?;

        Ok(connection)
    }
    async fn handshake(&mut self, key: &str) -> Result<(), std::io::Error> {
        handshake::handshake(key, &mut self.0).await
    }
    /// Read incoming data packets from tcp stream
    async fn incoming_message(&mut self) -> WsGonzaleResult<Message> {
        let mut buffer: [u8; 2] = [0; 2];

        // Do a peek-ahead so we can utilize the read_exact of the full payload and then use From<&[u8]> for Dataframe
        match self.0.peek(&mut buffer).await {
            // Connection was aborted
            Ok(s) if s == 0 => {
                return Err(std::io::Error::from(std::io::ErrorKind::ConnectionAborted))?;
            }
            // fin(126) + opcode for close(8), see rfc protocol.. just ignore the reason.
            Ok(_) if buffer.len() > 0 && buffer[0] == 136 => return Ok(Message::Close),
            Ok(_) => {}
            // Upon error, return early
            Err(err) => return Err(err)?,
        };

        // Just peek ahead for the largest data package 127 in size. That's 2 (two first frames including fin, rsv1-3, mask and payload_length) + 8 (u64 size in bytes) + 4 (masking_key) = 14
        let mut peeked_buff: [u8; 14] = [0; 14];
        self.0.peek(&mut peeked_buff).await?;

        let dataframe = dataframe::DataframeBuilder::new(peeked_buff.to_vec())?;
        let mut payload: Vec<u8> = vec![0; dataframe.get_full_frame_length() as usize];

        self.0.read_exact(&mut payload).await?;

        let dataframe = dataframe::DataframeBuilder::new(payload)?;
        let message = dataframe.get_message().unwrap_or(Message::Unknown);

        Ok(message)
    }
}

/// This Drop method around WsConnection is pretty neat. Makes sure we are notifying the developer created struct implemented WsClientHook that the
/// WsConnection has been dropped
impl Drop for WsEvents {
    fn drop(&mut self) {
        // Block this thread until notified since Drop doesn't support async
        self.client_hook.after_drop();
    }
}
