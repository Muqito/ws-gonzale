extern crate ws_gonzale;
pub use {
    crate::lib::server::ServerData,
    ws_gonzale::{async_std::sync::Arc, AsyncResult},
};

pub mod life_cycles;
pub mod server;

pub async fn start_server() -> AsyncResult<()> {
    let shared_data = Arc::new(ServerData::new());
    //<editor-fold desc="Client lifecycle">
    let arc_connections_shared_data = Arc::clone(&shared_data);
    let thread_connections = life_cycles::connections(arc_connections_shared_data);
    //</editor-fold>
    //<editor-fold desc="Server lifecycle">
    let tm_shared_data = Arc::clone(&shared_data);
    let thread_server = life_cycles::server(tm_shared_data);
    //</editor-fold>
    if let Err(err) = futures::try_join!(thread_connections, thread_server) {
        return Err(err);
    }

    Ok(())
}
