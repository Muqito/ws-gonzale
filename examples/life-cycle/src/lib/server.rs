pub use {
    std::collections::HashMap,
    ws_gonzale::{
        async_channel::{self, Receiver, Sender},
        async_std::sync::{Arc, Mutex},
        Channel, Channels, Message,
    },
};

pub enum ServerMessage {
    ClientMessage(Message),
    ClientJoined((u32, Channels)),
    ClientDisconnected(u32),
}
pub struct ServerData {
    channel: Channel<ServerMessage>,
    pub connections: Arc<Mutex<HashMap<u32, Channels>>>,
}
impl ServerData {
    pub fn new() -> Self {
        Self {
            channel: async_channel::unbounded(),
            connections: Default::default(),
        }
    }
    pub fn get_channel_receiver(&self) -> Receiver<ServerMessage> {
        self.channel.1.clone()
    }
    pub fn get_channel_sender(&self) -> Sender<ServerMessage> {
        self.channel.0.clone()
    }
    pub async fn get_nr_of_connections(&self) -> u64 {
        self.connections.lock().await.len() as u64
    }
}
