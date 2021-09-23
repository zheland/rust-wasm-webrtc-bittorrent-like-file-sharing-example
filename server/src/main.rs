#![warn(
    clippy::all,
    rust_2018_idioms,
    missing_copy_implementations,
    missing_debug_implementations,
    single_use_lifetimes,
    trivial_casts,
    unused_import_braces,
    unused_qualifications,
    unused_results
)]

mod app;
mod server;
mod socket;
mod socket_receiver;
mod socket_sender;
mod state;

use app::app;
use server::Server;
use socket::Socket;
use socket_receiver::{SocketMessageReceiveError, SocketReceiver};
use socket_sender::{SocketMessageSendError, SocketSender};
use state::State;

pub fn main() -> anyhow::Result<()> {
    use async_std::task;

    task::block_on(app())
}
