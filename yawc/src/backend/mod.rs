use core::unreachable;

#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;
#[cfg(feature = "x11")]
pub mod x11;

/// Type to represent available rendering backends
pub enum Backend {
    Udev,
    Winit,
    X11,
    NoBackend,
}

impl Backend {
    pub fn to_unit(&self) -> () {
        match self {
            Backend::Udev => self::udev::run_udev(),
            Backend::Winit => self::winit::run_winit(),
            Backend::X11 => self::x11::run_x11(),
            Backend::NoBackend => unreachable!(),
        }
    }
}

pub fn run_backend(backend: Backend) {
    backend.to_unit();
}

pub fn sugguest_useful_backend() -> Backend {
    #[cfg(debug_assertions)]
    let start = std::time::Instant::now();

    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        tracing::debug!("Detected wayland session");
        #[cfg(debug_assertions)]
        tracing::debug!("Check took {:?}", start.elapsed());
        return Backend::Winit;
    }

    if std::env::var_os("DISPLAY").is_some() {
        tracing::debug!("Detected x11 session");
        #[cfg(debug_assertions)]
        tracing::debug!("Check took {:?}", start.elapsed());
        return Backend::X11;
    }

    if unsafe { libc::isatty(libc::STDIN_FILENO) == 1 } {
        tracing::debug!("Detected tty → udev backend");
        #[cfg(debug_assertions)]
        tracing::debug!("Check took {:?}", start.elapsed());
        return Backend::Udev;
    }

    Backend::NoBackend
}
