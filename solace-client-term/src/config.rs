use std::{
    fs::{self},
    path::PathBuf,
};

use anyhow::Context;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub(crate) static CONFIG: Lazy<Config> =
    Lazy::new(|| Config::new().expect("Failed to load configuration"));

#[macro_export]
macro_rules! config {
    ($field:ident $(. $subfield:ident)*) => {
        &$crate::config::CONFIG.$field $(. $subfield)*
    };
}

#[macro_export]
macro_rules! config_hex_color {
    ($field:ident $(. $subfield:ident)*) => {
        $crate::color::hex_to_rgb($crate::config!($field $(. $subfield)*))
    };
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Config {
    pub(crate) colors: Colors,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Colors {
    pub(crate) bg: String,
    pub(crate) channel_mention: String,
    pub(crate) command: String,
    pub(crate) error_bg: String,
    pub(crate) error_fg: String,
    pub(crate) fg: String,
    pub(crate) message: String,
    pub(crate) prompt_nick: String,
    pub(crate) server_message: String,
    pub(crate) timestamp_bg: String,
    pub(crate) timestamp_fg: String,
    pub(crate) topic_bg: String,
    pub(crate) topic_fg: String,
    pub(crate) user_name: String,
    pub(crate) user_mention: String,
}

impl Config {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let base_path = xdg::BaseDirectories::with_prefix("solace")
            .with_context(|| "ERROR: Couldn't find XDG path for solace")?;
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
            }
        }

        let default_config = Self::default();
        let config_toml = toml::to_string_pretty(&default_config)
            .with_context(|| "ERROR: Failed to serialize default config")?;

        let config_path = base_path
            .place_config_file("config.toml")
            .with_context(|| "ERROR: Failed to create config directory")?;

        fs::write(&config_path, config_toml)
            .with_context(|| format!("ERROR: Failed to write default config to {config_path:?}"))?;

        Ok(default_config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            colors: Colors {
                bg: "#282a36".to_string(),
                fg: "#f8f8f2".to_string(),
                message: "#f8f8f2".to_string(),
                user_name: "#8be9fd".to_string(),
                user_mention: "#ffb86c".to_string(),
                channel_mention: "#50fa7b".to_string(),
                timestamp_bg: "#44475a".to_string(),
                timestamp_fg: "#bd93f9".to_string(),
                topic_bg: "#44475a".to_string(),
                topic_fg: "#ff79c6".to_string(),
                prompt_nick: "#8be9fd".to_string(),
                server_message: "#6272a4".to_string(),
                command: "#ff5555".to_string(),
                error_bg: "#ff5555".to_string(),
                error_fg: "#f8f8f2".to_string(),
            },
        }
    }
}
