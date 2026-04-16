use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) launcher: LauncherConfig,
    pub(crate) profile: ProfileConfig,
    pub(crate) auth: AuthConfig,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LauncherConfig {
    pub(crate) main_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProfileConfig {
    pub(crate) name: String,
    pub(crate) game_dir: PathBuf,
    pub(crate) version: String,
    pub(crate) loader: String,
    pub(crate) username: Option<String>,
    pub(crate) mods: ModsConfig,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AuthConfig {
    pub(crate) mode: AuthMode,
    pub(crate) client_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AuthMode {
    Msa,
    Offline,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ModsConfig {
    pub(crate) source_dir: PathBuf,
    pub(crate) mode: ModSyncMode,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ModSyncMode {
    Mirror,
    Merge,
}

pub(crate) fn load_config(path: &Path) -> Result<Config> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    let cfg: Config = toml::from_str(&text)
        .with_context(|| format!("failed to parse TOML in {}", path.display()))?;
    Ok(cfg)
}

pub(crate) fn validate_config(cfg: &Config) -> Result<()> {
    if cfg.profile.name.trim().is_empty() {
        bail!("profile.name cannot be empty");
    }
    if cfg.profile.version.trim().is_empty() {
        bail!("profile.version cannot be empty");
    }
    if cfg.profile.loader.trim().is_empty() {
        bail!("profile.loader cannot be empty");
    }
    if cfg.auth.mode == AuthMode::Msa
        && cfg
            .auth
            .client_id
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
    {
        bail!("auth.client_id cannot be empty when auth.mode is msa");
    }
    Ok(())
}

pub(crate) fn ensure_instance_dirs(cfg: &Config) -> Result<()> {
    fs::create_dir_all(&cfg.launcher.main_dir)
        .with_context(|| format!("failed to create {}", cfg.launcher.main_dir.display()))?;
    fs::create_dir_all(&cfg.profile.game_dir)
        .with_context(|| format!("failed to create {}", cfg.profile.game_dir.display()))?;
    Ok(())
}
