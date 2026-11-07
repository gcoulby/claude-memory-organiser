use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;

use crate::db::vault::{DbKey, VaultPaths};
use crate::error::{AppError, AppResult};

/// An unlocked vault: the key (for opening extra connections, such as the
/// import's) and the connection used by ordinary commands.
pub struct Session {
    pub key: DbKey,
    pub conn: Connection,
}

pub struct AppState {
    pub paths: VaultPaths,
    pub session: Mutex<Option<Session>>,
    pub import_running: Arc<AtomicBool>,
    pub import_cancel: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(paths: VaultPaths) -> Self {
        Self {
            paths,
            session: Mutex::new(None),
            import_running: Arc::new(AtomicBool::new(false)),
            import_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Locks the session mutex. A poisoned mutex only means another command
    /// panicked mid-way; the connection itself is still usable.
    pub fn session(&self) -> MutexGuard<'_, Option<Session>> {
        self.session.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Runs `f` with the open connection, or fails if the vault is locked.
    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let guard = self.session();
        let session = guard.as_ref().ok_or(AppError::Locked)?;
        f(&session.conn)
    }
}
