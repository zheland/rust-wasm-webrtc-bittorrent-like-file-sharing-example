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

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc<'_> = wee_alloc::WeeAlloc::INIT;

mod app_ui;
mod callback;
mod connection;
mod file;
mod html;
mod message;
mod params;
mod peer;
mod peer_ui;
mod tracker;

use app_ui::AppUi;
use callback::{init_weak_callback, Callback, ClosureCell0, ClosureCell1};
use connection::Connection;
use file::{File, FileSetChunkError};
use html::{body, window, ElementExt};
use message::PeerPeerMessage;
use params::{
    default_tracker_address, CHUNK_SIZE, DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES,
    DEFAULT_PEER_SEND_INTERVAL_MS, DEFAULT_UPLOAD_SPEED_BITS_PER_SECOND,
};
use peer::{Peer, PeerParams};
use peer_ui::PeerUi;
use tracker::Tracker;

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();
    let _: &mut _ = Box::leak(Box::new(AppUi::new()));
}
