use std::{
    fs::{self},
    path::PathBuf,
};

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    colors: Colors,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Colors {
    background: String,
    channel_mention: String,
    message: String,
    server_message: String,
    user_mention: String,
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

                    crate::log!("{config:?}");
                    return Ok(config);
                }
            } else {
                crate::log!("NO");
            }
        }

        anyhow::bail!("ERROR: No config file found!")
    }
}
