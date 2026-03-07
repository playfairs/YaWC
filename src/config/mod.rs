pub mod binds;
pub mod envs;

use binds::*;
use core::{convert::From, include_str, option::Option::None};
use envs::*;
use kdl::KdlDocument;
use std::{
    env,
    fs::{self, read_to_string},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, RwLock},
};

static CONFIG_INSTANCE: RwLock<Option<Arc<KdlDocument>>> = RwLock::new(None);

#[derive(Debug, Default, PartialEq)]
pub struct Config {
    pub env: Envs,
    pub binds: Binds,
}

impl Config {
    pub fn init_config_instance() {
        let mut mut_cinst = CONFIG_INSTANCE.write().unwrap();
        *mut_cinst = Some(Arc::new(get_config_string().parse().unwrap()))
    }

    /// Read `RwLock<Option<Arc<KdlDocument>>>` returning
    /// `Config`, make sure to run `init_config_instance()`
    /// before attempting to run this function.
    pub fn read_config() -> Self {
        let oinst = CONFIG_INSTANCE.read().unwrap();
        let _cinst = Arc::clone(
            &oinst
                .as_ref()
                .expect("init_config_instance() must be called first")
                .clone(),
        );

        Self {
            env: Envs(vec![Env {
                name: "TESTING".into(),
                value: "1".into(),
            }]),
            binds: Binds(vec![Bind {
                key: binds::Key {
                    trigger: binds::Trigger::Keysym(smithay::input::keyboard::Keysym::q),
                    modifiers: Modifiers::SUPER | Modifiers::SHIFT,
                },
                repeat: false,
                cooldown: None,
                allow_when_locked: false,
                action: binds::Action::Quit,
            }]),
        }
    }
}

fn get_config_string() -> String {
    if let Some(cpath) = get_config_path() {
        read_to_string(cpath).unwrap()
    } else {
        create_missing_config().unwrap();
        get_config_string()
    }
}

/// Tries to find the most reasonable path for YaWC to use.
/// This will trial `YAWC_CONFIG_PATH` before trying
/// `XDG_CONFIG_HOME` which then will try a system path which
/// will be `/etc/yawc/config.kdl`, if none are found it will
/// go ahead and return `None`.
fn get_config_path() -> Option<PathBuf> {
    // This will literally just expect $YAWC_CONFIG_PATH to be
    // full absolute path, an example of the ENV variable would
    // be YAWC_CONFIG_PATH=/home/invra/.yawc/config.kdl, which
    // if it exists, that will be the PathBuf.
    if let Ok(cfg) = env::var("YAWC_CONFIG_PATH")
        && let Some(p) = check_exists(cfg)
    {
        return Some(p);
    }

    // XDG config directory
    if let Ok(cfg) = env::var("XDG_CONFIG_HOME")
        && let Some(p) = check_exists(PathBuf::from(cfg).join("yawc/config.kdl"))
    {
        return Some(p);
    }

    // Default ~/.config fallback
    if let Ok(home) = env::var("HOME")
        && let Some(p) = check_exists(PathBuf::from(home).join(".config/yawc/config.kdl"))
    {
        return Some(p);
    }

    // System config
    if let Some(p) = check_exists("/etc/yawc/config.kdl") {
        return Some(p);
    }

    None
}

fn create_missing_config() -> Result<(), io::Error> {
    let path: PathBuf = if let Ok(user) = env::var("USER") {
        if user == "root" {
            PathBuf::from("/etc/yawc/config.kdl")
        } else if let Ok(xdg_conf_home) = env::var("XDG_CONFIG_HOME") {
            PathBuf::from(format!("{}/yawc/config.kdl", xdg_conf_home))
        } else if let Ok(home_dir) = env::var("HOME") {
            PathBuf::from(format!("{}/.config/yawc/config.kdl", home_dir))
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No config path found",
            ));
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "The USER env var was not found.",
        ));
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::File::create(&path)?;
    file.write_all(get_init_config_string().as_bytes())?;
    Ok(())
}

fn get_init_config_string() -> String {
    include_str!("./init.kdl").to_string()
}

fn check_exists(p: impl Into<PathBuf>) -> Option<PathBuf> {
    let p = p.into();
    p.exists().then_some(p)
}
