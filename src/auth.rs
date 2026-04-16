use anyhow::{Context, Result};
use portablemc::msa;

use crate::config::{AuthMode, Config};

#[derive(Debug)]
pub(crate) enum AuthSession {
    Offline,
    Msa(msa::Account),
}

pub(crate) fn authenticate(cfg: &Config) -> Result<AuthSession> {
    match cfg.auth.mode {
        AuthMode::Offline => {
            println!("Offline auth selected; skipping Microsoft login.");
            Ok(AuthSession::Offline)
        }
        AuthMode::Msa => authenticate_msa(cfg),
    }
}

fn authenticate_msa(cfg: &Config) -> Result<AuthSession> {
    let client_id = cfg
        .auth
        .client_id
        .as_deref()
        .map(str::trim)
        .filter(|client_id| !client_id.is_empty())
        .context("auth.client_id is required when auth.mode is msa")?;

    let db = msa::Database::new(cfg.launcher.main_dir.join("portablemc_msa.json"));

    if let Some(mut account) = load_msa_account(&db, client_id)? {
        if let Err(err) = account.request_refresh() {
            eprintln!("Stored Microsoft session could not be refreshed: {err}; requesting a new login.");
        } else {
            db.store(account.clone())
                .context("failed to store refreshed Microsoft session")?;
            println!("Reused Microsoft account '{}'.", account.username());
            return Ok(AuthSession::Msa(account));
        }
    }

    let auth = msa::Auth::new(client_id);
    let flow = auth
        .request_device_code()
        .context("failed to request Microsoft device code")?;

    println!("{}", flow.message());

    let account = flow.wait().context("Microsoft authentication failed")?;
    db.store(account.clone())
        .context("failed to store Microsoft account")?;
    println!("Authenticated as '{}'.", account.username());

    Ok(AuthSession::Msa(account))
}

fn load_msa_account(db: &msa::Database, client_id: &str) -> Result<Option<msa::Account>> {
    Ok(db
        .load_iter()
        .context("failed to load Microsoft account database")?
        .find(|account| account.app_id() == client_id))
}
