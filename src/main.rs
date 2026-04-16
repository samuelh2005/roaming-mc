mod auth;
mod config;
mod launcher;
mod mods;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::load_config(&cli.config)?;

    config::validate_config(&cfg)?;
    config::ensure_instance_dirs(&cfg)?;

    match cli.cmd {
        Command::Auth => {
            let _ = auth::authenticate(&cfg)?;
        }
        Command::SyncMods => {
            mods::sync_mods(&cfg)?;
            println!("Mods synced for profile '{}'.", cfg.profile.name);
        }
        Command::Install => {
            let auth_session = auth::authenticate(&cfg)?;
            mods::sync_mods(&cfg)?;
            let _game = launcher::install_game(&cfg, &auth_session)?;
            println!("Profile '{}' prepared.", cfg.profile.name);
            println!("PortableMC installation completed through the library.");
        }
        Command::Run { dry_run } => {
            let auth_session = auth::authenticate(&cfg)?;
            mods::sync_mods(&cfg)?;
            let game = launcher::install_game(&cfg, &auth_session)?;
            launcher::run_game(&game, dry_run)?;
        }
    }

    Ok(())
}