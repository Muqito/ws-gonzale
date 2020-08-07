pub use {
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
    ws_gonzale::{channel, Channel, Channels, Message, Receiver, Sender},
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
    pub fn get_channel_receiver(&self) -> Receiver<ServerMessage> {
        self.receiver.lock().unwrap().take().unwrap()
    }
    pub fn get_channel_sender(&self) -> Sender<ServerMessage> {
        self.sender.clone()
    }
    pub fn get_nr_of_connections(&self) -> u64 {
        self.connections.lock().unwrap().len() as u64
    }
}
