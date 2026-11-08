use std::{collections::HashMap, env, fs, path::PathBuf};

use adi2::midi::{ChannelParameterType, DeviceParameterType, Output};
use serde::Deserialize;

use crate::hooks::Hooks;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    active_output_hooks: HashMap<Output, Vec<String>>,
    channel_hooks: HashMap<ChannelParameterType, Vec<String>>,
    device_hooks: HashMap<DeviceParameterType, Vec<String>>,
}

impl Config {
    pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
        let config_file_path = Self::config_file_path();
        if !fs::exists(&config_file_path)? {
            return Ok(Config::default());
        }
        let config_text = fs::read_to_string(config_file_path).expect("Failed to read config file");
        Ok(toml::from_str(&config_text)?)
    }

    fn config_file_path() -> PathBuf {
        let config_home = match env::var_os("XDG_CONFIG_HOME") {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => {
                let home = env::var_os("HOME").expect("HOME or XDG_CONFIG_HOME must be set");
                PathBuf::from(home).join(".config")
            }
        };
        config_home.join("adi2d").join("config.toml")
    }

    pub fn hooks(&self) -> Hooks {
        Hooks::create(
            self.active_output_hooks.clone(),
            self.channel_hooks.clone(),
            self.device_hooks.clone()
        )
    }
}
