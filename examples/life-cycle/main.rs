mod lib;
use lib::start_server;
use ws_gonzale::{async_std::task, AsyncResult};

fn main() -> AsyncResult<()> {
    task::block_on(start_server())
}
