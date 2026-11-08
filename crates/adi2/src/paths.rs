use std::{env, path::{Path, PathBuf}};

use thiserror::Error;

#[derive(Debug, Error)]
#[error("no runtime directory found; set XDG_RUNTIME_DIR")]
pub struct RuntimeDirNotFound;

pub fn runtime_dir() -> Result<PathBuf, RuntimeDirNotFound> {
    let base = match env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => default_runtime_base().ok_or(RuntimeDirNotFound)?,
    };
    Ok(base.join("adi2d"))
}

pub fn socket_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("socket")
}

#[cfg(target_os = "macos")]
fn default_runtime_base() -> Option<PathBuf> {
    Some(env::temp_dir())
}

#[cfg(not(target_os = "macos"))]
fn default_runtime_base() -> Option<PathBuf> {
    None
}
