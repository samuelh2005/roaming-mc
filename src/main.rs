use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command as ProcCommand, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Parser, Debug)]
#[command(name = "roaming-mc")]
#[command(version)]
#[command(about = "A small Minecraft instance launcher")]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Auth,
    SyncMods,
    Install,
    Run {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Deserialize)]
struct Config {
    launcher: LauncherConfig,
    profile: ProfileConfig,
    auth: AuthConfig,
}

#[derive(Debug, Deserialize)]
struct LauncherConfig {
    portablemc_exe: PathBuf,
    main_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ProfileConfig {
    name: String,
    game_dir: PathBuf,
    version: String,
    loader: String,
    username: Option<String>,
    mods: ModsConfig,
}

#[derive(Debug, Deserialize)]
struct AuthConfig {
    mode: AuthMode,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum AuthMode {
    Msa,
    Offline,
}

#[derive(Debug, Deserialize)]
struct ModsConfig {
    source_dir: PathBuf,
    mode: ModSyncMode,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ModSyncMode {
    Mirror,
    Merge,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = load_config(&cli.config)?;

    validate_config(&cfg)?;
    ensure_instance_dirs(&cfg)?;

    match cli.cmd {
        Command::Auth => {
            auth(&cfg)?;
        }
        Command::SyncMods => {
            sync_mods(&cfg)?;
            println!("Mods synced for profile '{}'.", cfg.profile.name);
        }
        Command::Install => {
            auth(&cfg)?;
            sync_mods(&cfg)?;
            println!("Profile '{}' prepared.", cfg.profile.name);
            println!("PortableMC will handle Minecraft install on first launch.");
        }
        Command::Run { dry_run } => {
            auth(&cfg)?;
            sync_mods(&cfg)?;

            let args = launch_args(&cfg);
            if dry_run {
                println!("Would run:");
                println!("{:?}", cfg.launcher.portablemc_exe);
                println!("{}", args.join(" "));
            } else {
                run_portablemc(&cfg.launcher.portablemc_exe, &args)?;
            }
        }
    }

    Ok(())
}

fn load_config(path: &Path) -> Result<Config> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    let cfg: Config = toml::from_str(&text)
        .with_context(|| format!("failed to parse TOML in {}", path.display()))?;
    Ok(cfg)
}

fn validate_config(cfg: &Config) -> Result<()> {
    if cfg.profile.name.trim().is_empty() {
        bail!("profile.name cannot be empty");
    }
    if cfg.profile.version.trim().is_empty() {
        bail!("profile.version cannot be empty");
    }
    if cfg.profile.loader.trim().is_empty() {
        bail!("profile.loader cannot be empty");
    }
    if !cfg.launcher.portablemc_exe.exists() {
        bail!(
            "portablemc_exe not found: {}",
            cfg.launcher.portablemc_exe.display()
        );
    }
    Ok(())
}

fn ensure_instance_dirs(cfg: &Config) -> Result<()> {
    fs::create_dir_all(&cfg.launcher.main_dir)
        .with_context(|| format!("failed to create {}", cfg.launcher.main_dir.display()))?;
    fs::create_dir_all(&cfg.profile.game_dir)
        .with_context(|| format!("failed to create {}", cfg.profile.game_dir.display()))?;
    Ok(())
}

fn auth(cfg: &Config) -> Result<()> {
    match cfg.auth.mode {
        AuthMode::Offline => {
            println!("Offline auth selected; skipping Microsoft login.");
            Ok(())
        }
        AuthMode::Msa => {
            let auth_db = cfg.launcher.main_dir.join("portablemc_msa.json");
            if auth_db.exists() {
                return Ok(());
            }

            let mut cmd = ProcCommand::new(&cfg.launcher.portablemc_exe);
            cmd.arg("--main-dir")
                .arg(&cfg.launcher.main_dir)
                .arg("auth")
                .arg("login")
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());

            let status = cmd.status().context("failed to start PortableMC auth")?;
            if !status.success() {
                bail!("PortableMC auth failed");
            }
            Ok(())
        }
    }
}

fn sync_mods(cfg: &Config) -> Result<()> {
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

fn launch_args(cfg: &Config) -> Vec<String> {
    let mut args = vec![
        "--main-dir".into(),
        cfg.launcher.main_dir.display().to_string(),
        "start".into(),
        "--mc-dir".into(),
        cfg.profile.game_dir.display().to_string(),
    ];

    match cfg.auth.mode {
        AuthMode::Msa => {
            args.push("--auth".into());
        }
        AuthMode::Offline => {}
    }

    let username = cfg
        .profile
        .username
        .as_deref()
        .unwrap_or("Player")
        .to_string();

    args.push("--username".into());
    args.push(username);
    args.push(format!("{}:{}", cfg.profile.loader, cfg.profile.version));

    args
}

fn run_portablemc(exe: &Path, args: &[String]) -> Result<()> {
    let mut cmd = ProcCommand::new(exe);
    cmd.args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status().with_context(|| {
        format!(
            "failed to launch PortableMC: {} {}",
            exe.display(),
            args.join(" ")
        )
    })?;

    if !status.success() {
        bail!("PortableMC exited with a non-zero status");
    }

    Ok(())
}