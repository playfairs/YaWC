#![cfg_attr(
    not(any(feature = "winit", feature = "x11", feature = "udev")),
    allow(dead_code, unused_imports)
)]

#[cfg(any(feature = "udev", feature = "xwayland"))]
pub mod cursor;
pub mod drawing;
pub mod focus;
pub mod input_handler;
pub mod render;
pub mod shell;
pub mod state;
#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;
#[cfg(feature = "x11")]
pub mod x11;

pub use state::{ClientState, YawcState};

#[cfg(feature = "profile-with-tracy-mem")]
#[global_allocator]
static GLOBAL: profiling::tracy_client::ProfiledAllocator<std::alloc::System> =
    profiling::tracy_client::ProfiledAllocator::new(std::alloc::System, 10);

use yawc_config::Config;

fn main() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt()
            .compact()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().compact().init();
    }

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

    #[cfg(feature = "profile-with-puffin")]
    let _server =
        puffin_http::Server::new(&format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT)).unwrap();
    #[cfg(feature = "profile-with-puffin")]
    profiling::puffin::set_scopes_on(true);

    tracing::info!("Initialising configuration instance");
    Config::init_config_instance().unwrap();

    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        tracing::info!("Starting yawc with winit backend");
        winit::run_winit();
    } else if std::env::var("DISPLAY").is_ok() {
        tracing::info!("Starting yawc with x11 backend");
        x11::run_x11();
    } else {
        tracing::info!("Starting yawc on a tty using udev");
        udev::run_udev();
    }
}
