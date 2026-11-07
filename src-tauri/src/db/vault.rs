//! Password handling for the encrypted database.
//!
//! The SQLCipher key is derived from the password with Argon2id. Alongside the
//! database sits `vault.json` with the salt, the KDF parameters and a verifier.
//! The verifier lets us tell "wrong password" apart from "damaged database",
//! which SQLCipher reports identically. It is a hash of the derived key, so it
//! is no easier to attack than the database itself: the cost is in Argon2.

use std::fs;
use std::path::{Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};

const VERIFIER_CONTEXT: &[u8] = b"memory-organiser:key-verifier:v1";
const MIN_PASSWORD_CHARS: usize = 8;

/// Where the vault lives on disk. Everything is under one directory.
#[derive(Debug, Clone)]
pub struct VaultPaths {
    dir: PathBuf,
}

impl VaultPaths {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn vault_file(&self) -> PathBuf {
        self.dir.join("vault.json")
    }

    pub fn db_file(&self) -> PathBuf {
        self.dir.join("memory.db")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.dir.join("backups")
    }

    pub fn exists(&self) -> bool {
        self.vault_file().exists()
    }
}

/// Argon2id cost parameters. Stored in the vault file so they can be raised
/// later without breaking existing vaults.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// About half a second on a recent laptop.
    pub const DEFAULT: KdfParams = KdfParams {
        m_cost_kib: 64 * 1024,
        t_cost: 3,
        p_cost: 1,
    };
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultFile {
    format: u32,
    kdf: String,
    params: KdfParams,
    salt: String,
    verifier: String,
}

/// The raw 256-bit SQLCipher key. Wiped from memory on drop.
#[derive(Clone)]
pub struct DbKey(Zeroizing<[u8; 32]>);

impl DbKey {
    /// The value for `PRAGMA key`, using SQLCipher's raw-key syntax so
    /// SQLCipher does not run its own (weaker) PBKDF2 on top.
    pub fn pragma(&self) -> Zeroizing<String> {
        Zeroizing::new(format!("PRAGMA key = \"x'{}'\";", hex::encode(*self.0)))
    }

    fn verifier(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(VERIFIER_CONTEXT);
        hasher.update(*self.0);
        hex::encode(hasher.finalize())
    }

    #[cfg(test)]
    pub fn for_tests(byte: u8) -> Self {
        DbKey(Zeroizing::new([byte; 32]))
    }
}

impl std::fmt::Debug for DbKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DbKey(..)")
    }
}

fn derive(password: &str, salt: &[u8], params: KdfParams) -> AppResult<DbKey> {
    let params = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(32))
        .map_err(|e| AppError::Kdf(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(password.as_bytes(), salt, out.as_mut())
        .map_err(|e| AppError::Kdf(e.to_string()))?;
    Ok(DbKey(out))
}

/// Creates a new vault and returns the key for the (not yet created) database.
pub fn create(paths: &VaultPaths, password: &str, params: KdfParams) -> AppResult<DbKey> {
    if paths.exists() || paths.db_file().exists() {
        return Err(AppError::VaultExists);
    }
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(AppError::InvalidInput(format!(
            "Use at least {MIN_PASSWORD_CHARS} characters."
        )));
    }

    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| AppError::Kdf(e.to_string()))?;
    let key = derive(password, &salt, params)?;

    let file = VaultFile {
        format: 1,
        kdf: "argon2id".into(),
        params,
        salt: hex::encode(salt),
        verifier: key.verifier(),
    };
    fs::create_dir_all(&paths.dir)?;
    write_atomic(&paths.vault_file(), &serde_json::to_vec_pretty(&file)?)?;
    Ok(key)
}

/// Checks the password against the vault and returns the database key.
pub fn unlock(paths: &VaultPaths, password: &str) -> AppResult<DbKey> {
    if !paths.exists() {
        return Err(AppError::NoVault);
    }
    let file: VaultFile = serde_json::from_slice(&fs::read(paths.vault_file())?)?;
    if file.format != 1 || file.kdf != "argon2id" {
        return Err(AppError::Corrupt(
            "the vault file uses an unknown format".into(),
        ));
    }
    let salt = hex::decode(&file.salt)
        .map_err(|_| AppError::Corrupt("the vault file has an invalid salt".into()))?;
    let key = derive(password, &salt, file.params)?;
    if key.verifier() != file.verifier {
        return Err(AppError::WrongPassword);
    }
    Ok(key)
}

/// Writes to a temporary file then renames, so a crash never leaves a
/// half-written vault file (which would lock the user out).
fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cheap parameters so the tests stay fast. Never used in the app.
    const TEST_PARAMS: KdfParams = KdfParams {
        m_cost_kib: 256,
        t_cost: 1,
        p_cost: 1,
    };

    #[test]
    fn right_password_unlocks_and_gives_same_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = VaultPaths::new(dir.path().to_path_buf());
        let created = create(&paths, "correct horse", TEST_PARAMS).expect("create");
        let unlocked = unlock(&paths, "correct horse").expect("unlock");
        assert_eq!(*created.0, *unlocked.0);
    }

    #[test]
    fn wrong_password_is_reported_as_such() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = VaultPaths::new(dir.path().to_path_buf());
        create(&paths, "correct horse", TEST_PARAMS).expect("create");
        assert!(matches!(
            unlock(&paths, "battery staple"),
            Err(AppError::WrongPassword)
        ));
    }

    #[test]
    fn refuses_short_password_and_second_vault() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = VaultPaths::new(dir.path().to_path_buf());
        assert!(matches!(
            create(&paths, "short", TEST_PARAMS),
            Err(AppError::InvalidInput(_))
        ));
        create(&paths, "long enough", TEST_PARAMS).expect("create");
        assert!(matches!(
            create(&paths, "long enough", TEST_PARAMS),
            Err(AppError::VaultExists)
        ));
    }
}
