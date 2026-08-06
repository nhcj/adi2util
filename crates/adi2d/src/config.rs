use std::{collections::HashMap, env, fmt, fs, path::PathBuf, process};

use adi2::midi::{ChannelParameterType, DeviceParameterType, Output};
use log::error;
use serde::Deserialize;
use strfmt::strfmt;

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
        let config_dir = env::var("XDG_CONFIG_HOME")
            .map(|s| PathBuf::from(format!("{}/adi2d", s)))
            .expect("XDG_CONFIG_HOME must be set");
        config_dir.join("config.toml")
    }

    // probably not great to have these in two places...
    pub fn hooks(&self) -> Hooks {
        Hooks {
            active_output_hooks: self.active_output_hooks.clone(),
            channel_parameter_hooks: self.channel_hooks.clone(),
            device_parameter_hooks: self.device_hooks.clone(),
        }
    }
}

// need to rethink the api for this, probably simplify it
pub struct Hooks {
    active_output_hooks: HashMap<Output, Vec<String>>,
    channel_parameter_hooks: HashMap<ChannelParameterType, Vec<String>>,
    device_parameter_hooks: HashMap<DeviceParameterType, Vec<String>>,
}

impl Hooks {
    pub fn run_active_output_hooks(&self, old: Option<Output>, new: Output) {
        let old = old.map(|o| o.to_string()).unwrap_or_else(|| "-".to_string());
        run_hooks(self.active_output_hooks.get(&new), "ActiveOutput", old, new);
    }

    pub fn run_channel_hooks(&self, parameter: ChannelParameterType, old: impl fmt::Display, new: impl fmt::Display) {
        run_hooks(self.channel_parameter_hooks.get(&parameter), parameter, old, new);
    }

    pub fn run_device_hooks(&self, parameter: DeviceParameterType, old: impl fmt::Display, new: impl fmt::Display) {
        run_hooks(self.device_parameter_hooks.get(&parameter), parameter, old, new);
    }
}

fn run_hooks(hooks: Option<&Vec<String>>, parameter: impl fmt::Display, old: impl fmt::Display, new: impl fmt::Display) {
    let Some(hooks) = hooks else { return; };
    let mut values = HashMap::new();
    values.insert("old".to_string(), old.to_string());
    values.insert("new".to_string(), new.to_string());
    for hook in hooks {
        let hook = match strfmt(hook, &values) {
            Ok(h) => h,
            Err(e) => {
                error!("failed to format hook ({}) with values: {:?}", hook, e);
                continue;
            }
        };
        let output = process::Command::new("sh")
            .arg("-c")
            .arg(hook)
            .output();
        match output {
            Ok(out) if !out.status.success() => {
                let err = String::from_utf8_lossy(&out.stderr);
                error!("hook for {} returned an error: {}", parameter, err);
            }
            Err(e) => { error!("failed to run hook for {}: {:?}", parameter, e); }
            _ => {}
        }
    }
}
