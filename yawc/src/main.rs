#![cfg_attr(
    not(any(feature = "winit", feature = "x11", feature = "udev")),
    allow(dead_code, unused_imports)
)]

pub mod backend;
#[cfg(any(feature = "udev", feature = "xwayland"))]
pub mod cursor;
pub mod drawing;
pub mod focus;
pub mod input_handler;
pub mod render;
pub mod shell;
pub mod state;

pub use state::{ClientState, YawcState};

#[cfg(feature = "profile-with-tracy-mem")]
#[global_allocator]
static GLOBAL: profiling::tracy_client::ProfiledAllocator<std::alloc::System> =
    profiling::tracy_client::ProfiledAllocator::new(std::alloc::System, 10);

use yawc_config::Config;

// #region agent log
fn agent_debug_log(
    hypothesis_id: &'static str,
    location: &'static str,
    message: &'static str,
    data: &str,
) {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/home/invra/dev/10-rs/yawc-epoch/.cursor/debug.log")
    {
        let _ = writeln!(
            f,
            r#"{{"id":"yawc_main_{}_{}","timestamp":{},"location":"{}","message":"{}","data":{},"runId":"pre-fix","hypothesisId":"{}"}}"#,
            std::process::id(),
            seq,
            timestamp,
            location,
            message,
            data,
            hypothesis_id
        );
    }
}
// #endregion

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

    // #region agent log
    agent_debug_log(
        "A",
        "compositor/yawc/src/main.rs:backend_select",
        "yawc starting, env presence",
        &format!(
            r#"{{"WAYLAND_DISPLAY":{},"DISPLAY":{},"XDG_CURRENT_DESKTOP":{},"XDG_SESSION_TYPE":{}}}"#,
            std::env::var("WAYLAND_DISPLAY").is_ok(),
            std::env::var("DISPLAY").is_ok(),
            std::env::var("XDG_CURRENT_DESKTOP")
                .ok()
                .map(|_| true)
                .unwrap_or(false),
            std::env::var("XDG_SESSION_TYPE")
                .ok()
                .map(|_| true)
                .unwrap_or(false)
        ),
    );
    // #endregion

    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        tracing::info!("Starting yawc with winit backend");
        backend::winit::run_winit();
    } else if std::env::var("DISPLAY").is_ok() {
        tracing::info!("Starting yawc with x11 backend");
        backend::x11::run_x11();
    } else {
        tracing::info!("Starting yawc on a tty using udev");
        backend::udev::run_udev();
    }
}
