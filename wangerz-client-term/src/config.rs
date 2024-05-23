use std::{
    fs::{self},
    path::PathBuf,
};

use anyhow::Context;
use once_cell::sync::Lazy;
use serde::Deserialize;

pub(crate) static CONFIG: Lazy<Config> =
    Lazy::new(|| Config::new().expect("Failed to load configuration"));

#[macro_export]
macro_rules! config {
    ($field:ident) => {
        &$crate::config::CONFIG.$field
    };
    ($field:ident.$subfield:ident) => {
        &$crate::config::CONFIG.$field.$subfield
    };
}

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) colors: Colors,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Colors {
    pub(crate) background: String,
    pub(crate) channel_mention: String,
    pub(crate) command: String,
    pub(crate) foreground: String,
    pub(crate) message: String,
    pub(crate) server_message: String,
    pub(crate) user_mention: String,
}

impl Config {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let base_path = xdg::BaseDirectories::with_prefix("wangerz")
            .with_context(|| "ERROR: Couldn't find XDG path for wangerz")?;
        let valid_config_paths = vec!["config.toml", ".config.toml"];

        for path in &valid_config_paths {
            if let Some(full_path) = base_path.find_config_file(path) {
                if PathBuf::from(&full_path).exists() {
                    let config_raw = fs::read_to_string(full_path)
                        .with_context(|| "ERROR: Failed to read file: {path:?}")?;
                    let config: Config = toml::from_str(&config_raw)
                        .with_context(|| "ERROR: Failed to parse {path:?}")?;

                    return Ok(config);
                }
            } else {
                crate::log!("NO");
            }
        }

        anyhow::bail!("ERROR: No config file found!")
    }
}
