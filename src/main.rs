#[cfg(feature = "profile-with-tracy-mem")]
#[global_allocator]
static GLOBAL: profiling::tracy_client::ProfiledAllocator<std::alloc::System> =
    profiling::tracy_client::ProfiledAllocator::new(std::alloc::System, 10);

use yawc::config::Config;

fn main() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt()
            .compact()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().compact().init();
    }

    let mut pargs = pico_args::Arguments::from_env();

    if pargs.contains(["-V", "--version"]) {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }

    let remaining = pargs.finish();
    if !remaining.is_empty() {
        eprintln!("Unknown arguments: {remaining:?}");
        std::process::exit(2);
    }

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
        yawc::winit::run_winit();
    } else if std::env::var("DISPLAY").is_ok() {
        tracing::info!("Starting yawc with x11 backend");
        yawc::x11::run_x11();
    } else {
        tracing::info!("Starting yawc on a tty using udev");
        yawc::udev::run_udev();
    }
}
