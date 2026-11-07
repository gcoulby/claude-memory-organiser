//! Opening the encrypted database, migrations and backups.

pub mod organise;
pub mod queries;
pub mod search;
pub mod vault;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};

use crate::error::{AppError, AppResult};
use vault::DbKey;

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Append only. Never edit or reorder a migration once released: installed
/// databases have already run it.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "tagging",
        sql: include_str!("migrations/0002_tagging.sql"),
    },
];

/// How many backups of each kind to keep.
pub const BACKUPS_KEPT: usize = 5;

/// Automatic and manual backups rotate separately, so pressing "Back up
/// now" a few times can never push out the copy taken before a migration.
#[derive(Debug, Clone, Copy)]
pub enum BackupKind {
    BeforeMigration,
    Manual,
}

impl BackupKind {
    fn prefix(self) -> &'static str {
        match self {
            BackupKind::BeforeMigration => "memory-",
            BackupKind::Manual => "manual-",
        }
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Opens (or creates) the database, applying any pending migrations.
pub fn open(path: &Path, key: &DbKey, backups_dir: &Path) -> AppResult<Connection> {
    open_with(path, key, backups_dir, MIGRATIONS)
}

pub fn open_with(
    path: &Path,
    key: &DbKey,
    backups_dir: &Path,
    migrations: &[Migration],
) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    conn.execute_batch(&key.pragma())?;

    // SQLCipher only checks the key on first read. The vault has already
    // confirmed the password, so a failure here means a damaged file.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|e| AppError::Corrupt(e.to_string()))?;

    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;

    migrate(&mut conn, path, backups_dir, migrations)?;
    Ok(conn)
}

fn migrate(
    conn: &mut Connection,
    path: &Path,
    backups_dir: &Path,
    migrations: &[Migration],
) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
           version     INTEGER PRIMARY KEY,
           name        TEXT NOT NULL,
           applied_at  INTEGER NOT NULL,
           app_version TEXT NOT NULL
         ) STRICT;",
    )?;

    let current: i64 = conn
        .query_row("SELECT max(version) FROM schema_migrations", [], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .optional()?
        .flatten()
        .unwrap_or(0);
    let supported = migrations.last().map_or(0, |m| m.version);

    if current > supported {
        return Err(AppError::NewerDatabase {
            found: current,
            supported,
        });
    }

    let pending: Vec<&Migration> = migrations.iter().filter(|m| m.version > current).collect();
    if pending.is_empty() {
        return Ok(());
    }
    // A brand new database has nothing worth backing up.
    if current > 0 {
        backup(
            conn,
            path,
            backups_dir,
            BackupKind::BeforeMigration,
            &format!("before-v{}", pending[0].version),
        )?;
    }

    for m in pending {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at, app_version)
             VALUES (?1, ?2, ?3, ?4)",
            (m.version, m.name, now_ms(), env!("CARGO_PKG_VERSION")),
        )?;
        tx.commit()?;
    }
    Ok(())
}

/// Copies the database file into the backups folder and prunes old copies.
/// The copy stays encrypted with the same key. Callers must make sure no
/// other connection is writing.
pub fn backup(
    conn: &Connection,
    path: &Path,
    backups_dir: &Path,
    kind: BackupKind,
    label: &str,
) -> AppResult<PathBuf> {
    fs::create_dir_all(backups_dir)?;
    // Fold the WAL into the main file so the single-file copy is complete.
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;

    // Zero-padded millis plus a sequence number keep names sorting in
    // creation order, even when two backups land in the same millisecond.
    let prefix = format!("{}{:013}-", kind.prefix(), now_ms());
    let seq = fs::read_dir(backups_dir)?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .count();
    let dest = backups_dir.join(format!("{prefix}{seq:04}-{label}.db"));
    fs::copy(path, &dest)?;
    prune_backups(backups_dir, kind, BACKUPS_KEPT)?;
    Ok(dest)
}

fn prune_backups(dir: &Path, kind: BackupKind, keep: usize) -> AppResult<()> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(kind.prefix()) && n.ends_with(".db"))
        })
        .collect();
    files.sort();
    let excess = files.len().saturating_sub(keep);
    for old in files.into_iter().take(excess) {
        fs::remove_file(old)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("memory.db");
        let backups = dir.path().join("backups");
        (dir, db, backups)
    }

    fn backup_count(dir: &Path) -> usize {
        fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
    }

    #[test]
    fn creates_schema_with_fts5() {
        let (_d, db, backups) = temp();
        let conn = open(&db, &DbKey::for_tests(1), &backups).expect("open");
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'messages_fts'",
                [],
                |r| r.get(0),
            )
            .expect("query");
        assert_eq!(n, 1);
        // A new database needs no backup.
        assert_eq!(backup_count(&backups), 0);
    }

    #[test]
    fn file_is_encrypted_and_needs_the_key() {
        let (_d, db, backups) = temp();
        {
            let conn = open(&db, &DbKey::for_tests(1), &backups).expect("open");
            conn.execute("INSERT INTO meta VALUES ('canary', 'plain words')", [])
                .expect("insert");
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
                .expect("checkpoint");
        }
        let bytes = fs::read(&db).expect("read");
        assert!(!bytes.windows(11).any(|w| w == b"plain words"));
        assert!(!bytes.starts_with(b"SQLite format 3"));

        let wrong = open(&db, &DbKey::for_tests(2), &backups);
        assert!(matches!(wrong, Err(AppError::Corrupt(_))));
    }

    #[test]
    fn reopening_is_a_no_op() {
        let (_d, db, backups) = temp();
        drop(open(&db, &DbKey::for_tests(1), &backups).expect("open"));
        drop(open(&db, &DbKey::for_tests(1), &backups).expect("reopen"));
        assert_eq!(backup_count(&backups), 0);
    }

    const M1: Migration = Migration {
        version: 1,
        name: "one",
        sql: "CREATE TABLE t (v TEXT NOT NULL) STRICT;",
    };
    const M2: Migration = Migration {
        version: 2,
        name: "two",
        sql: "ALTER TABLE t ADD COLUMN w TEXT;",
    };

    #[test]
    fn new_migration_backs_up_first_and_keeps_data() {
        let (_d, db, backups) = temp();
        let key = DbKey::for_tests(3);
        {
            let conn = open_with(&db, &key, &backups, &[M1]).expect("v1");
            conn.execute("INSERT INTO t (v) VALUES ('kept')", [])
                .expect("insert");
        }
        let conn = open_with(&db, &key, &backups, &[M1, M2]).expect("v2");
        let v: String = conn
            .query_row("SELECT v FROM t", [], |r| r.get(0))
            .expect("row");
        assert_eq!(v, "kept");
        assert_eq!(backup_count(&backups), 1);

        // The backup opens with the same key and holds the pre-migration data.
        let backup = fs::read_dir(&backups)
            .expect("dir")
            .next()
            .expect("one backup")
            .expect("entry")
            .path();
        let old = Connection::open(&backup).expect("open backup");
        old.execute_batch(&key.pragma()).expect("key");
        let version: i64 = old
            .query_row("SELECT max(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .expect("version");
        assert_eq!(version, 1);
    }

    #[test]
    fn refuses_database_from_newer_version() {
        let (_d, db, backups) = temp();
        let key = DbKey::for_tests(4);
        drop(open_with(&db, &key, &backups, &[M1, M2]).expect("v2"));
        let result = open_with(&db, &key, &backups, &[M1]);
        assert!(matches!(
            result,
            Err(AppError::NewerDatabase {
                found: 2,
                supported: 1
            })
        ));
    }

    #[test]
    fn keeps_only_the_last_few_backups() {
        let (_d, db, backups) = temp();
        let conn = open(&db, &DbKey::for_tests(5), &backups).expect("open");
        let mut made = Vec::new();
        for i in 0..(BACKUPS_KEPT + 3) {
            made.push(
                backup(
                    &conn,
                    &db,
                    &backups,
                    BackupKind::BeforeMigration,
                    &format!("t{i}"),
                )
                .expect("backup"),
            );
        }
        assert_eq!(backup_count(&backups), BACKUPS_KEPT);
        // The newest ones survive.
        for p in made.iter().rev().take(BACKUPS_KEPT) {
            assert!(p.exists());
        }
    }

    #[test]
    fn manual_backups_never_evict_migration_backups() {
        let (_d, db, backups) = temp();
        let conn = open(&db, &DbKey::for_tests(6), &backups).expect("open");
        let auto = backup(
            &conn,
            &db,
            &backups,
            BackupKind::BeforeMigration,
            "before-v2",
        )
        .expect("auto");
        for _ in 0..(BACKUPS_KEPT + 2) {
            backup(&conn, &db, &backups, BackupKind::Manual, "manual").expect("manual");
        }
        assert!(auto.exists());
        assert_eq!(backup_count(&backups), BACKUPS_KEPT + 1);
    }
}
