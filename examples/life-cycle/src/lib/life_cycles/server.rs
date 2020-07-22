use {
    crate::lib::{
        server::{
            ServerMessage,
            ServerData
        },
    },
    ws_gonzale::{
        Message,
        get_buffer,
        async_std::{
            task::{
                self,
                JoinHandle,
            },
            sync::Arc,
        },
    },
};

/// Takes care of basic `server-lifecycle`
/// This handles ClientJoined, ClientDisconnected and ClientMessages for now.
pub fn server(server_data: Arc<ServerData>) -> JoinHandle<Result<(), std::io::Error>> {
    task::spawn(async move {
        let receiver = server_data.get_channel_receiver();
        while let Ok(server_message) = receiver.recv().await {
            match server_message {
                // The client passed the Message packet to the server
                ServerMessage::ClientMessage(message) => {
                    // Ooh, the client sent a Text frame.. how exciting; send it to the other clients on the server
                    if let Message::Text(_) = message {
                        let buffer = get_buffer(message);
                        let connections = server_data.connections.lock().await;
                        for (_id, sender) in connections.iter() {
                            let _ = sender.send(buffer.clone()).await;
                        }
                    }
                },
                // Client unfortunately left the server. if you want you can notify the other clients on the server
                ServerMessage::ClientDisconnected(id) => {
                    server_data.connections.lock().await.remove(&id);
                    let total = server_data.get_nr_of_connections().await;
                    println!("Client: {} left, current total: {}", id, total);
                }
                // Welcome my dear friend; someone has joined our beloved server
                ServerMessage::ClientJoined((id, channel)) => {
                    server_data.connections.lock().await.insert(id, channel);
                    let total = server_data.get_nr_of_connections().await;
                    println!("Client: {} joined, current total: {}", id, total);
                }
            }
        }
        Ok::<(), std::io::Error>(())
    })
}