use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::error::AppResult;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub conversations: i64,
    pub messages: i64,
    pub last_import_at: Option<i64>,
}

pub fn library_stats(conn: &Connection) -> AppResult<LibraryStats> {
    let conversations = conn.query_row("SELECT count(*) FROM conversations", [], |r| r.get(0))?;
    let messages = conn.query_row("SELECT count(*) FROM messages", [], |r| r.get(0))?;
    let last_import_at = conn
        .query_row("SELECT max(finished_at) FROM imports", [], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .optional()?
        .flatten();
    Ok(LibraryStats {
        conversations,
        messages,
        last_import_at,
    })
}
