use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::config::{Config, ModSyncMode};

pub(crate) fn sync_mods(cfg: &Config) -> Result<()> {
    let src = &cfg.profile.mods.source_dir;
    if !src.exists() {
        bail!("mods source directory does not exist: {}", src.display());
    }
    if !src.is_dir() {
        bail!("mods source path is not a directory: {}", src.display());
    }

    let dst = cfg.profile.game_dir.join("mods");

    match cfg.profile.mods.mode {
        ModSyncMode::Mirror => sync_mods_mirror(src, &dst)?,
        ModSyncMode::Merge => sync_mods_merge(src, &dst)?,
    }

    Ok(())
}

fn sync_mods_mirror(src: &Path, dst: &Path) -> Result<()> {
    let staging_parent = dst
        .parent()
        .context("mods destination has no parent directory")?;
    let staging = unique_temp_dir(staging_parent, "mods-staging")?;

    copy_tree(src, &staging).with_context(|| {
        format!(
            "failed to copy mods from {} to staging dir {}",
            src.display(),
            staging.display()
        )
    })?;

    if dst.exists() {
        fs::remove_dir_all(dst)
            .with_context(|| format!("failed to remove old mods dir {}", dst.display()))?;
    }

    fs::rename(&staging, dst).with_context(|| {
        format!(
            "failed to replace {} with staged mods {}",
            dst.display(),
            staging.display()
        )
    })?;

    Ok(())
}

fn sync_mods_merge(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("failed to create mods dir {}", dst.display()))?;
    copy_tree(src, dst).with_context(|| {
        format!(
            "failed to merge mods from {} into {}",
            src.display(),
            dst.display()
        )
    })?;
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("failed to create {}", dst.display()))?;

    for entry in fs::read_dir(src).with_context(|| format!("failed to read {}", src.display()))? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_tree(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn unique_temp_dir(parent: &Path, prefix: &str) -> Result<PathBuf> {
    for attempt in 0..1000u32 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before UNIX_EPOCH")?
            .as_nanos();
        let candidate = parent.join(format!("{prefix}-{stamp}-{attempt}"));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to create temp dir {}", candidate.display()))
            }
        }
    }

    bail!("failed to create a unique temporary directory")
}
