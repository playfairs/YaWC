#![cfg_attr(
    not(any(feature = "winit", feature = "x11", feature = "udev")),
    allow(dead_code, unused_imports)
)]

pub mod backend;
#[cfg(any(feature = "udev", feature = "xwayland"))]
pub mod cursor;
pub mod drawing;
pub mod focus;
pub mod handlers;
pub mod logging;

pub mod render;
pub mod shell;
pub mod state;

pub use state::{ClientState, YawcState};

#[cfg(feature = "profile-with-tracy")]
#[global_allocator]
static GLOBAL: profiling::tracy_client::ProfiledAllocator<std::alloc::System> =
    profiling::tracy_client::ProfiledAllocator::new(std::alloc::System, 10);

use crate::backend::{run_backend, sugguest_useful_backend};
use yawc_config::Config;

fn main() {
    tracing::subscriber::set_global_default(logging::SimpleSubscriber).unwrap();
    let mut args = std::env::args().skip(1);
    let mut unknown = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-V" | "--version" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                return;
            }
            _ => unknown.push(arg),
        }
    }

    if !unknown.is_empty() {
        eprintln!("Unknown arguments: {unknown:?}");
        std::process::exit(2);
    }

    drop(args);
    drop(unknown);

    #[cfg(feature = "profile-with-tracy")]
    profiling::tracy_client::Client::start();
    profiling::register_thread!("Main Thread");

    tracing::info!("Initialising configuration instance");
    Config::init_config_instance().unwrap();

    run_backend(sugguest_useful_backend());
}
