pub mod binds;
pub mod envs;
pub mod xkb;

use binds::*;
use core::{convert::From, include_str, option::Option::None, result::Result::Err};
use envs::*;
use std::{
    env,
    fs::{self, read_to_string},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, RwLock},
};
use xkb::*;

static CONFIG_INSTANCE: RwLock<Option<Arc<Config>>> = RwLock::new(None);

#[derive(knus::Decode, Debug)]
pub struct RawConfig {
    #[knus(child, unwrap(argument))]
    pub version: Option<String>,
    #[knus(child)]
    pub envs: Option<Envs>,
    #[knus(child)]
    pub binds: Option<Binds>,
    #[knus(child)]
    pub xkb: Option<RawXkb>,
}

#[derive(Debug)]
pub struct Config {
    pub version: i16,
    pub envs: Envs,
    pub binds: Binds,
    pub xkb: Xkb,
}

impl From<RawConfig> for Config {
    fn from(raw: RawConfig) -> Self {
        Self {
            version: raw
                .version
                .unwrap_or_else(|| "-1".into())
                .parse::<i16>()
                .expect("version is meant to represent a i16 value"),
            envs: raw.envs.unwrap_or_else(|| Envs(vec![])),
            binds: raw.binds.unwrap_or_else(|| Binds(vec![])),
            xkb: Xkb::from(raw.xkb.unwrap_or_else(|| RawXkb {
                layout: Some("us".into()),
                variant: None,
                options: None,
                repeat_rate: Some("50".into()),
                repeat_delay: Some("200".into()),
            })),
        }
    }
}

fn get_config_instance() -> PathBuf {
    if let Some(cpath) = get_config_path() {
        cpath
    } else {
        create_missing_config().unwrap();
        get_config_instance()
    }
}

fn parse_config(path_ref: impl AsRef<str>) -> Result<Config, ()> {
    let path: &str = path_ref.as_ref();

    if let Ok(text) = read_to_string(path) {
        return Ok(Config::from(knus::parse::<RawConfig>(path, &text).unwrap()));
    } else {
        return Err(());
    };
}

impl Config {
    pub fn init_config_instance() -> Result<(), ()> {
        let config = parse_config(
            get_config_instance()
                .into_os_string()
                .into_string()
                .unwrap(),
        )
        .expect("config parse failed");

        tracing::info!("Config: {config:?}");

        match config.version {
            -1 => {
                tracing::warn!("Configuration version is unset! Defaulting to v1 specification")
            }
            1 => {} // Config verison value is Ok.
            _ => tracing::warn!(
                "Configuration version is set to an unknown value! Defaulting to v1 specification"
            ),
        }

        Ok(*CONFIG_INSTANCE.write().unwrap() = Some(Arc::new(config)))
    }

    /// Read `RwLock<Option<Arc<KdlDocument>>>` returning
    /// `Config`, make sure to run `init_config_instance()`
    /// before attempting to run this function.
    pub fn read_config() -> Arc<Config> {
        CONFIG_INSTANCE
            .read()
            .unwrap()
            .as_ref()
            .expect("init_config_instance() must be called first")
            .clone()
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
    file.write_all(include_str!("../../resources/init.kdl").as_bytes())?;
    Ok(())
}

fn check_exists(p: impl Into<PathBuf>) -> Option<PathBuf> {
    let p = p.into();
    p.exists().then_some(p)
}
