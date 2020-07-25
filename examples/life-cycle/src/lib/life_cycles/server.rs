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
                        //<editor-fold desc="One way, wait for each client before moving on to the next one">
                        /*                        let buffer = get_buffer(message);
                        let connections = server_data.connections.lock().await;

                        for (_id, (_mpmc_channel, tcp_stream)) in connections.iter() {
                            // clone would be needed even though the lifetime seems OK in this scope.
                            // But remember; we are sending it through a mpmc channel
                            // In theory; this could be run in a hundred days.
                            // So writing to the tcp_stream which we don't need to clone it before sending and writing to tcp_stream is more performant.
                            // Sending to a mpmc is really quick; though we do have to clone the buffer then which is slow.
                            // let _ = _mpmc_channel.send(buffer.clone()).await;
                            let _ = tcp_stream.to_owned().write_all(&buffer).await;
                        }*/
                        //</editor-fold>
                        // <editor-fold desc="Async way directly to the tcp_stream">
                        let pinned_buffer = Arc::new(get_buffer(message));
                        let connections = server_data.connections.lock().await;

                        // Create all these tasks and then join them and wait for all of them to finish; instead of sending to one client at a time.
                        let handles: Vec<_> = connections
                            .iter()
                            .map(|(_id, (_mpmc_channel, tcp_stream))| {
                                let mut tcp_stream_owned = tcp_stream.clone();
                                let pinned_buffer = pinned_buffer.clone();
                                task::spawn(async move {
                                    // Await the task so we make sure it's sent to the client.
                                    let _ = tcp_stream_owned.write_all(&pinned_buffer).await;
                                    Ok::<_, ()>(())
                                })
                            })
                            .collect();

                        let _ = futures::future::join_all(handles).await;
                        // </editor-fold>
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
