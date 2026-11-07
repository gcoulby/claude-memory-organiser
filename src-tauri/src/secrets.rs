//! The Anthropic API key lives in the OS keychain (macOS Keychain, Windows
//! Credential Manager), never in a file or the database.

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.gcoulby.memoryorganiser";
const API_KEY_ACCOUNT: &str = "anthropic-api-key";

fn entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, API_KEY_ACCOUNT).map_err(keychain_error)
}

fn keychain_error(err: keyring::Error) -> AppError {
    AppError::Keychain(err.to_string())
}

pub fn set_api_key(key: &str) -> AppResult<()> {
    let key = key.trim();
    if !key.starts_with("sk-ant-") {
        return Err(AppError::InvalidInput(
            "That does not look like an Anthropic API key (they start with sk-ant-).".into(),
        ));
    }
    entry()?.set_password(key).map_err(keychain_error)
}

pub fn api_key() -> AppResult<Option<String>> {
    match entry()?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(keychain_error(e)),
    }
}

pub fn clear_api_key() -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(keychain_error(e)),
    }
}
