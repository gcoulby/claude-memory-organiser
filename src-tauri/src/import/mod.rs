//! Importing an export into the database.
//!
//! Rules (see CLAUDE.md):
//! - Idempotent, keyed on conversation and message uuid. An unchanged
//!   conversation is skipped by hash.
//! - User-owned columns (`state`, `excluded`) are never written after insert.
//! - Nothing is ever deleted. Messages missing from a later export stay.
//! - Unknown fields survive in `raw_json`.
//! - One transaction per import: cancel or failure leaves the database as it was.

pub mod export;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use crate::tree::{Node, main_path};
use export::{EntryKind, ExportSink};

/// Parent id the export uses for a root message.
const ROOT_PARENT: &str = "00000000-0000-4000-8000-000000000000";
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub file: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub conversations: i64,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub conversations_seen: i64,
    pub conversations_added: i64,
    pub conversations_updated: i64,
    pub conversations_unchanged: i64,
    pub conversations_skipped: i64,
    pub messages_added: i64,
    pub messages_updated: i64,
    pub messages_skipped: i64,
    pub files_stored: i64,
    pub duration_ms: i64,
}

pub fn run_import(
    conn: &mut Connection,
    paths: &[PathBuf],
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(&ImportProgress),
) -> AppResult<ImportSummary> {
    let started = Instant::now();
    let sources = export::collect_sources(paths)?;
    let names: Vec<String> = sources.iter().map(|p| export::display_name(p)).collect();

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO imports (started_at, sources) VALUES (?1, ?2)",
        (now_ms(), serde_json::to_string(&names)?),
    )?;
    let import_id = tx.last_insert_rowid();

    let mut importer = Importer {
        tx: &tx,
        import_id,
        cancel,
        on_progress,
        summary: ImportSummary::default(),
        last_progress: None,
    };
    export::read_sources(&sources, &mut importer)?;
    let mut summary = importer.summary;
    summary.duration_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);

    tx.execute(
        "UPDATE imports SET finished_at = ?1, conversations_seen = ?2,
           conversations_added = ?3, conversations_updated = ?4,
           messages_added = ?5, messages_updated = ?6
         WHERE id = ?7",
        (
            now_ms(),
            summary.conversations_seen,
            summary.conversations_added,
            summary.conversations_updated,
            summary.messages_added,
            summary.messages_updated,
            import_id,
        ),
    )?;
    tx.commit()?;
    Ok(summary)
}

struct Importer<'a> {
    tx: &'a Transaction<'a>,
    import_id: i64,
    cancel: &'a AtomicBool,
    on_progress: &'a mut dyn FnMut(&ImportProgress),
    summary: ImportSummary,
    last_progress: Option<Instant>,
}

impl ExportSink for Importer<'_> {
    fn conversation(&mut self, value: Value) -> AppResult<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        self.summary.conversations_seen += 1;
        self.import_conversation(value)
    }

    fn side_file(&mut self, path: &str, kind: &EntryKind, content: String) -> AppResult<()> {
        let kind = match kind {
            EntryKind::Project => "project",
            EntryKind::DesignChat => "design_chat",
            _ => return Ok(()),
        };
        let stored = self.tx.execute(
            "INSERT OR IGNORE INTO export_files (path, kind, content_hash, content, first_import_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (path, kind, sha256_hex(content.as_bytes()), &content, self.import_id),
        )?;
        self.summary.files_stored += i64::try_from(stored).unwrap_or(i64::MAX);
        Ok(())
    }

    fn progress(&mut self, file: &str, bytes_done: u64, bytes_total: u64) {
        let due = self
            .last_progress
            .is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL);
        if due || bytes_done >= bytes_total {
            self.last_progress = Some(Instant::now());
            (self.on_progress)(&ImportProgress {
                file: file.to_owned(),
                bytes_done,
                bytes_total,
                conversations: self.summary.conversations_seen,
            });
        }
    }
}

impl Importer<'_> {
    fn import_conversation(&mut self, value: Value) -> AppResult<()> {
        let hash = sha256_hex(&serde_json::to_vec(&value)?);
        let Value::Object(mut conv) = value else {
            self.summary.conversations_skipped += 1;
            return Ok(());
        };
        let Some(uuid) = str_field(&conv, "uuid").filter(|u| !u.is_empty()) else {
            self.summary.conversations_skipped += 1;
            return Ok(());
        };
        let uuid = uuid.to_owned();

        let existing: Option<String> = self
            .tx
            .prepare_cached("SELECT raw_hash FROM conversations WHERE uuid = ?1")?
            .query_row([&uuid], |r| r.get(0))
            .optional()?;
        if existing.as_deref() == Some(hash.as_str()) {
            self.summary.conversations_unchanged += 1;
            return Ok(());
        }

        let messages = match conv.remove("chat_messages") {
            Some(Value::Array(m)) => m,
            _ => Vec::new(),
        };
        let created_at = str_field(&conv, "created_at")
            .unwrap_or_default()
            .to_owned();
        let updated_at = str_field(&conv, "updated_at")
            .map(str::to_owned)
            .unwrap_or_else(|| created_at.clone());
        let name = str_field(&conv, "name").unwrap_or_default().to_owned();

        // `state` and `excluded` are deliberately absent from the update:
        // they belong to the user.
        let conv_id: i64 = self
            .tx
            .prepare_cached(
                "INSERT INTO conversations
                   (uuid, name, created_at, updated_at, raw_hash, raw_json, first_import_id, last_import_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT (uuid) DO UPDATE SET
                   name = excluded.name,
                   created_at = excluded.created_at,
                   updated_at = excluded.updated_at,
                   raw_hash = excluded.raw_hash,
                   raw_json = excluded.raw_json,
                   last_import_id = excluded.last_import_id
                 RETURNING id",
            )?
            .query_row(
                (
                    &uuid,
                    &name,
                    &created_at,
                    &updated_at,
                    &hash,
                    serde_json::to_string(&conv)?,
                    self.import_id,
                ),
                |r| r.get(0),
            )?;
        if existing.is_some() {
            self.summary.conversations_updated += 1;
        } else {
            self.summary.conversations_added += 1;
        }

        self.import_messages(conv_id, &created_at, &messages)?;
        recompute_main_path(self.tx, conv_id)?;
        Ok(())
    }

    fn import_messages(
        &mut self,
        conv_id: i64,
        conv_created: &str,
        messages: &[Value],
    ) -> AppResult<()> {
        let known: HashSet<String> = self
            .tx
            .prepare_cached("SELECT uuid FROM messages WHERE conversation_id = ?1")?
            .query_map([conv_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;

        let mut upsert = self.tx.prepare_cached(
            "INSERT INTO messages
               (conversation_id, uuid, parent_uuid, sender, text, attachment_text,
                created_at, updated_at, array_idx, raw_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT (conversation_id, uuid) DO UPDATE SET
               parent_uuid = excluded.parent_uuid,
               sender = excluded.sender,
               text = excluded.text,
               attachment_text = excluded.attachment_text,
               created_at = excluded.created_at,
               updated_at = excluded.updated_at,
               array_idx = excluded.array_idx,
               raw_json = excluded.raw_json
             WHERE messages.raw_json IS NOT excluded.raw_json",
        )?;

        for (array_idx, m) in messages.iter().enumerate() {
            let Some(obj) = m.as_object() else {
                self.summary.messages_skipped += 1;
                continue;
            };
            let Some(uuid) = str_field(obj, "uuid").filter(|u| !u.is_empty()) else {
                self.summary.messages_skipped += 1;
                continue;
            };
            let parent = str_field(obj, "parent_message_uuid")
                .filter(|p| !p.is_empty() && *p != ROOT_PARENT);
            let changed = upsert.execute((
                conv_id,
                uuid,
                parent,
                str_field(obj, "sender").unwrap_or("unknown"),
                message_text(obj),
                attachment_text(obj),
                str_field(obj, "created_at").unwrap_or(conv_created),
                str_field(obj, "updated_at"),
                i64::try_from(array_idx).unwrap_or(i64::MAX),
                serde_json::to_string(m)?,
            ))?;
            if changed > 0 {
                if known.contains(uuid) {
                    self.summary.messages_updated += 1;
                } else {
                    self.summary.messages_added += 1;
                }
            }
        }
        Ok(())
    }
}

fn str_field<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Text for search and tagging, built from the `text` content blocks.
///
/// The top-level `text` field is a flattened rendering that includes
/// placeholders for tool calls, so it is only a fallback for messages that
/// have no content blocks at all (older exports may lack them).
pub fn message_text(msg: &Map<String, Value>) -> String {
    match msg.get("content").and_then(Value::as_array) {
        Some(blocks) if !blocks.is_empty() => blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => str_field(msg, "text").unwrap_or_default().to_owned(),
    }
}

/// Pasted and uploaded file text, with file names, for search.
pub fn attachment_text(msg: &Map<String, Value>) -> String {
    let Some(items) = msg.get("attachments").and_then(Value::as_array) else {
        return String::new();
    };
    items
        .iter()
        .filter_map(|a| {
            let name = a
                .get("file_name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let body = a
                .get("extracted_content")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let joined = format!("{name}\n{body}");
            (!joined.trim().is_empty()).then_some(joined)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn recompute_main_path(tx: &Transaction, conv_id: i64) -> AppResult<()> {
    let (ids, nodes): (Vec<i64>, Vec<Node>) = tx
        .prepare_cached(
            "SELECT id, uuid, parent_uuid, created_at, array_idx
             FROM messages WHERE conversation_id = ?1",
        )?
        .query_map([conv_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                Node {
                    uuid: r.get(1)?,
                    parent: r.get(2)?,
                    created_at: r.get(3)?,
                    array_idx: r.get(4)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .unzip();
    let path = main_path(&nodes);

    tx.prepare_cached(
        "UPDATE messages SET idx = NULL WHERE conversation_id = ?1 AND idx IS NOT NULL",
    )?
    .execute([conv_id])?;
    let mut set_idx = tx.prepare_cached("UPDATE messages SET idx = ?1 WHERE id = ?2")?;
    for (pos, &i) in path.iter().enumerate() {
        set_idx.execute((i64::try_from(pos).unwrap_or(i64::MAX), ids[i]))?;
    }
    tx.prepare_cached("UPDATE conversations SET message_count = ?1 WHERE id = ?2")?
        .execute((i64::try_from(path.len()).unwrap_or(i64::MAX), conv_id))?;
    Ok(())
}

#[cfg(test)]
mod tests;
