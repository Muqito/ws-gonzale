use {
    crate::lib::server::{ServerData, ServerMessage},
    futures::AsyncWriteExt,
    std::sync::Arc,
    ws_gonzale::{
        async_std::task::{self, JoinHandle},
        get_buffer, Message,
    },
};

/// Takes care of basic `server-lifecycle`
/// This handles ClientJoined, ClientDisconnected and ClientMessages for now.
pub fn server(server_data: Arc<ServerData>) -> JoinHandle<Result<(), std::io::Error>> {
    task::spawn(async move {
        let mut receiver = server_data.get_channel_receiver();
        while let Some(server_message) = receiver.recv() {
            match server_message {
                // The client passed the Message packet to the server
                ServerMessage::ClientMessage(message) => {
                    // Ooh, the client sent a Text frame.. how exciting; send it to the other clients on the server
                    if let Message::Text(_) = message {
                        //<editor-fold desc="One way, wait for each client before moving on to the next one">
                        let buffer = get_buffer(message);
                        let connections = server_data.connections.lock().unwrap();

                        for (_id, (mpmc_channel, _tcp_stream)) in connections.iter() {
                            // clone would be needed even though the lifetime seems OK in this scope.
                            // But remember; we are sending it through a mpmc channel
                            // In theory; this could be run in a hundred days.
                            // So writing to the tcp_stream which we don't need to clone it before sending and writing to tcp_stream is more performant.
                            // Sending to a mpmc is really quick; though we do have to clone the buffer then which is slow.
                            let _ = mpmc_channel.send(buffer.clone());
                        }
                        // </editor-fold>
                    }
                }
                // Client unfortunately left the server. if you want you can notify the other clients on the server
                ServerMessage::ClientDisconnected(id) => {
                    server_data.connections.lock().unwrap().remove(&id);
                    let total = server_data.get_nr_of_connections();
                    // println!("Client: {} left, current total: {}", id, total);
                    if total == 0 {
                        println!("Server is now empty of clients");
                    }
                }
                // Welcome my dear friend; someone has joined our beloved server
                ServerMessage::ClientJoined((id, channels)) => {
                    server_data.connections.lock().unwrap().insert(id, channels);

                    // A nice welcome message once everything is setup, we are making sure this is the first thing the users see because of the await.
                    if let Some((_mpmc, tcp_stream)) =
                        server_data.connections.lock().unwrap().get_mut(&id)
                    {
                        let _ = task::block_on(tcp_stream.write_all(&get_buffer(Message::Text(
                            "Welcome to the server!".to_string(),
                        ))));
                    };
                    /*                    let total = server_data.get_nr_of_connections().await;
                    println!("Client: {} joined, current total: {}", id, total);*/
                }
            }
        }
        Ok::<(), std::io::Error>(())
    })
}
