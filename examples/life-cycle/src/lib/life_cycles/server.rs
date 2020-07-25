use {
    crate::lib::server::{ServerData, ServerMessage},
    futures::AsyncWriteExt,
    ws_gonzale::{
        async_std::{
            sync::Arc,
            task::{self, JoinHandle},
        },
        get_buffer, Message,
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

                        for (_id, (_mpmc_channel, tcp_stream)) in connections.iter() {
                            // clone would be needed even though the lifetime seems OK in this scope.
                            // But remember; we are sending it through a mpmc channel
                            // In theory; this could be run in a hundred days.
                            // So writing to the tcp_stream which we don't need to clone it before sending and writing to tcp_stream is more performant.
                            // let _ = _mpmc_channel.send(buffer.clone()).await;
                            let _ = tcp_stream.to_owned().write_all(&buffer).await;
                        }
                    }
                }
                // Client unfortunately left the server. if you want you can notify the other clients on the server
                ServerMessage::ClientDisconnected(id) => {
                    server_data.connections.lock().await.remove(&id);
                    let total = server_data.get_nr_of_connections().await;
                    println!("Client: {} left, current total: {}", id, total);
                }
                // Welcome my dear friend; someone has joined our beloved server
                ServerMessage::ClientJoined((id, channels)) => {
                    server_data.connections.lock().await.insert(id, channels);
                    let total = server_data.get_nr_of_connections().await;
                    println!("Client: {} joined, current total: {}", id, total);
                }
            }
        }
        Ok::<(), std::io::Error>(())
    })
}
