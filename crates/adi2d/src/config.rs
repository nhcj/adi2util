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
    midi: MidiConfig,
}

// durations are in milliseconds
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct MidiConfig {
    pub poll_interval: u64,
    pub scan_interval: u64,
    // the device answers status polls within milliseconds, so a few seconds of silence means it's gone
    pub response_timeout: u64,
    // exit (for a service manager to restart the daemon) instead of reconnecting when the
    // device hasn't answered for response_timeout, counting from startup
    pub exit_when_unavailable: bool,
}

impl Default for MidiConfig {
    fn default() -> Self {
        MidiConfig {
            poll_interval: 1000,
            scan_interval: 1000,
            response_timeout: 3000,
            exit_when_unavailable: false,
        }
    }
}

impl MidiConfig {
    fn validate(&self) -> Result<(), String> {
        if self.poll_interval == 0 || self.scan_interval == 0 {
            return Err("midi.poll-interval and midi.scan-interval must be greater than 0".to_string());
        }
        if self.response_timeout <= self.poll_interval {
            return Err("midi.response-timeout must be greater than midi.poll-interval".to_string());
        }
        Ok(())
    }
}

impl Config {
    pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
        let config_file_path = Self::config_file_path();
        if !fs::exists(&config_file_path)? {
            return Ok(Config::default());
        }
        let config_text = fs::read_to_string(config_file_path).expect("Failed to read config file");
        let config: Config = toml::from_str(&config_text)?;
        config.midi.validate()?;
        Ok(config)
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

    pub fn midi(&self) -> MidiConfig {
        self.midi
    }
}
