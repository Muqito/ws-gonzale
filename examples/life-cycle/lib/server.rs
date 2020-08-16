pub use {
    std::collections::HashMap,
    ws_gonzale::{
        async_std::sync::{Arc, Mutex},
        channel, Channel, Channels, Message, Receiver, Sender,
    },
};
pub enum ServerMessage {
    ClientMessage(Message),
    ClientJoined((u32, Channels)),
    ClientDisconnected(u32),
}
pub struct ServerData {
    sender: Sender<ServerMessage>,
    receiver: Arc<Mutex<Option<Receiver<ServerMessage>>>>,
    pub connections: Arc<Mutex<HashMap<u32, Channels>>>,
}
impl ServerData {
    pub fn new() -> Self {
        let (sender, receiver) = channel::unbounded();
        Self {
            sender,
            receiver: Arc::new(Mutex::new(Some(receiver))),
            connections: Default::default(),
        }
    }
    pub async fn get_channel_receiver(&self) -> Receiver<ServerMessage> {
        self.receiver.lock().await.take().unwrap()
    }
    pub fn get_channel_sender(&self) -> Sender<ServerMessage> {
        self.sender.clone()
    }
    pub async fn get_nr_of_connections(&self) -> u64 {
        self.connections.lock().await.len() as u64
    }
}
