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
mod file_ui;
mod html;
mod interval_handler;
mod params;
mod peer_ui;
mod rand_ext;
mod sender;
mod time;

use app_ui::AppUi;
use callback::{init_weak_callback, ClosureCell1};
use file_ui::FileUi;
use html::{body, ElementExt};
use interval_handler::{IntervalHandler, NewIntervalHandlerError};
use params::{
    default_tracker_address, DEFAULT_MAX_DATACHANNEL_BUFFER_BYTES, DEFAULT_PEER_DATA_SEND_INTERVAL,
    DEFAULT_PIECE_RESEND_INTERVAL, DEFAULT_STATE_RESEND_INTERVAL,
    DEFAULT_UPLOAD_SPEED_BYTES_PER_SECOND,
};
use peer_ui::PeerUi;
use rand_ext::JsRandom;
use sender::{Sender, SenderParams};
use time::{monotonic_time, Time};

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Trace).unwrap();
    let _: &mut _ = Box::leak(Box::new(AppUi::new()));
}
