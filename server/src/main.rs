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

use app::app;

pub fn main() -> anyhow::Result<()> {
    use async_std::task;

    task::block_on(app())
}
