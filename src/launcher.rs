use anyhow::{bail, Context, Result};
use portablemc::{base, fabric, forge, moj};

use crate::{
    auth::AuthSession,
    config::Config,
};

pub(crate) fn install_game(cfg: &Config, auth_session: &AuthSession) -> Result<base::Game> {
    let loader = cfg.profile.loader.trim().to_ascii_lowercase();

    match loader.as_str() {
        "mojang" | "vanilla" | "release" | "snapshot" => install_mojang_game(cfg, auth_session),
        "fabric" | "quilt" | "legacyfabric" | "legacy-fabric" | "legacy_fabric" | "babric" => {
            install_fabric_game(cfg, auth_session, &loader)
        }
        "forge" | "neoforge" => install_forge_game(cfg, auth_session, &loader),
        _ => bail!(
            "unsupported loader '{}'; expected mojang, vanilla, fabric, quilt, legacyfabric, babric, forge, or neoforge",
            cfg.profile.loader
        ),
    }
}

pub(crate) fn run_game(game: &base::Game, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would run:");
        println!("{:?}", game.command());
        return Ok(());
    }

    let status = game
        .spawn_and_wait()
        .context("failed to launch Minecraft")?;

    if !status.success() {
        bail!("PortableMC exited with a non-zero status");
    }

    Ok(())
}

fn install_mojang_game(cfg: &Config, auth_session: &AuthSession) -> Result<base::Game> {
    let version = match cfg.profile.version.trim().to_ascii_lowercase().as_str() {
        "release" => moj::Version::Release,
        "snapshot" => moj::Version::Snapshot,
        _ => moj::Version::Name(cfg.profile.version.clone()),
    };

    let mut installer = moj::Installer::new(version);
    configure_common_paths(installer.base_mut(), cfg);
    configure_auth(&mut installer, cfg, auth_session);

    installer
        .install(())
        .context("PortableMC failed to install the Mojang version")
}

fn install_fabric_game(
    cfg: &Config,
    auth_session: &AuthSession,
    loader_name: &str,
) -> Result<base::Game> {
    let loader = parse_fabric_loader(loader_name)?;
    let mut installer = fabric::Installer::new_with_stable(loader);

    installer.set_game_version(cfg.profile.version.clone());
    configure_common_paths(installer.mojang_mut().base_mut(), cfg);
    configure_auth(installer.mojang_mut(), cfg, auth_session);

    installer
        .install(())
        .context("PortableMC failed to install the Fabric-compatible version")
}

fn install_forge_game(
    cfg: &Config,
    auth_session: &AuthSession,
    loader_name: &str,
) -> Result<base::Game> {
    let loader = parse_forge_loader(loader_name)?;
    let mut installer = forge::Installer::new(loader, forge::Version::Stable(cfg.profile.version.clone()));

    configure_common_paths(installer.mojang_mut().base_mut(), cfg);
    configure_auth(installer.mojang_mut(), cfg, auth_session);

    installer
        .install(())
        .context("PortableMC failed to install the Forge-compatible version")
}

fn configure_common_paths(installer: &mut base::Installer, cfg: &Config) {
    installer.set_main_dir(cfg.launcher.main_dir.clone());
    installer.set_mc_dir(cfg.profile.game_dir.clone());
    installer.set_bin_dir(cfg.launcher.main_dir.join("bin"));
}

fn configure_auth(installer: &mut moj::Installer, cfg: &Config, auth_session: &AuthSession) {
    match auth_session {
        AuthSession::Offline => {
            let username = cfg.profile.username.as_deref().unwrap_or("Player");
            installer.set_auth_offline_username(username.to_owned());
        }
        AuthSession::Msa(account) => {
            installer.set_auth_msa(account);
        }
    }
}

fn parse_fabric_loader(loader: &str) -> Result<fabric::Loader> {
    match loader {
        "fabric" => Ok(fabric::Loader::Fabric),
        "quilt" => Ok(fabric::Loader::Quilt),
        "legacyfabric" | "legacy-fabric" | "legacy_fabric" => Ok(fabric::Loader::LegacyFabric),
        "babric" => Ok(fabric::Loader::Babric),
        _ => bail!("unsupported Fabric-like loader: {loader}"),
    }
}

fn parse_forge_loader(loader: &str) -> Result<forge::Loader> {
    match loader {
        "forge" => Ok(forge::Loader::Forge),
        "neoforge" => Ok(forge::Loader::NeoForge),
        _ => bail!("unsupported Forge-like loader: {loader}"),
    }
}
