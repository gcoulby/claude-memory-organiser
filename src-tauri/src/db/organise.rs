//! Reading and editing the organised layer: themes, segments, memory items.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeRow {
    pub id: i64,
    pub name: String,
    pub origin: String,
    pub segments: i64,
    pub conversations: i64,
    pub last_active: Option<String>,
}

/// Live themes (merged ones fold into their target), biggest first.
pub fn list_themes(conn: &Connection) -> AppResult<Vec<ThemeRow>> {
    let rows = conn
        .prepare_cached(
            "SELECT t.id, t.name, t.origin,
                    count(DISTINCT st.segment_id),
                    count(DISTINCT s.conversation_id),
                    max(c.updated_at)
             FROM themes t
             LEFT JOIN segment_themes st ON st.theme_id = t.id
             LEFT JOIN segments s ON s.id = st.segment_id
             LEFT JOIN conversations c ON c.id = s.conversation_id
             WHERE t.merged_into IS NULL
             GROUP BY t.id
             ORDER BY count(DISTINCT st.segment_id) DESC, t.name COLLATE NOCASE",
        )?
        .query_map([], |r| {
            Ok(ThemeRow {
                id: r.get(0)?,
                name: r.get(1)?,
                origin: r.get(2)?,
                segments: r.get(3)?,
                conversations: r.get(4)?,
                last_active: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSegment {
    pub segment_id: i64,
    pub conversation_id: i64,
    pub conversation_name: String,
    pub conversation_updated_at: String,
    pub title: String,
    pub summary: String,
    pub intent: String,
    /// Uuid of the segment's first message, to jump straight to it.
    pub start_uuid: Option<String>,
    pub confidence: Option<f64>,
}

pub fn theme_segments(conn: &Connection, theme_id: i64) -> AppResult<Vec<ThemeSegment>> {
    let rows = conn
        .prepare_cached(
            "SELECT s.id, s.conversation_id, c.name, c.updated_at, s.title, s.summary, s.intent,
                    (SELECT m.uuid FROM messages m
                     WHERE m.conversation_id = s.conversation_id AND m.idx = s.start_idx),
                    st.confidence
             FROM segment_themes st
             JOIN segments s ON s.id = st.segment_id
             JOIN conversations c ON c.id = s.conversation_id
             WHERE st.theme_id = ?1
             ORDER BY c.updated_at DESC, s.start_idx",
        )?
        .query_map([theme_id], |r| {
            Ok(ThemeSegment {
                segment_id: r.get(0)?,
                conversation_id: r.get(1)?,
                conversation_name: r.get(2)?,
                conversation_updated_at: r.get(3)?,
                title: r.get(4)?,
                summary: r.get(5)?,
                intent: r.get(6)?,
                start_uuid: r.get(7)?,
                confidence: r.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentInfo {
    pub id: i64,
    pub start_idx: i64,
    pub end_idx: i64,
    pub title: String,
    pub summary: String,
    pub intent: String,
    pub source: String,
    pub themes: Vec<String>,
    pub entities: Vec<String>,
}

pub fn conversation_segments(
    conn: &Connection,
    conversation_id: i64,
) -> AppResult<Vec<SegmentInfo>> {
    let mut segments: Vec<SegmentInfo> = conn
        .prepare_cached(
            "SELECT id, start_idx, end_idx, title, summary, intent, source
             FROM segments WHERE conversation_id = ?1 ORDER BY start_idx",
        )?
        .query_map([conversation_id], |r| {
            Ok(SegmentInfo {
                id: r.get(0)?,
                start_idx: r.get(1)?,
                end_idx: r.get(2)?,
                title: r.get(3)?,
                summary: r.get(4)?,
                intent: r.get(5)?,
                source: r.get(6)?,
                themes: Vec::new(),
                entities: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    for seg in &mut segments {
        seg.themes = conn
            .prepare_cached(
                "SELECT t.name FROM segment_themes st JOIN themes t ON t.id = st.theme_id
                 WHERE st.segment_id = ?1 ORDER BY st.confidence DESC",
            )?
            .query_map([seg.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        seg.entities = conn
            .prepare_cached(
                "SELECT t.name FROM segment_tags st JOIN tags t ON t.id = st.tag_id
                 WHERE st.segment_id = ?1 ORDER BY t.name COLLATE NOCASE",
            )?
            .query_map([seg.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
    }
    Ok(segments)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Suggested,
    Confirmed,
    Dismissed,
}

impl MemoryStatus {
    fn as_str(self) -> &'static str {
        match self {
            MemoryStatus::Suggested => "suggested",
            MemoryStatus::Confirmed => "confirmed",
            MemoryStatus::Dismissed => "dismissed",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRow {
    pub id: i64,
    pub kind: String,
    pub text: String,
    pub status: String,
    pub conversation_id: i64,
    pub conversation_name: String,
    /// The first source message, to jump to it.
    pub source_uuid: Option<String>,
    pub source_on_main_path: bool,
    /// When the source message was written: how long a loop has been open.
    pub source_at: Option<String>,
}

/// Memory items that have not been dismissed, oldest open loops first.
pub fn list_memory(conn: &Connection) -> AppResult<Vec<MemoryRow>> {
    let rows = conn
        .prepare_cached(
            "SELECT mi.id, mi.kind, mi.text, mi.status, mi.conversation_id, c.name,
                    m.uuid, m.idx IS NOT NULL, m.created_at
             FROM memory_items mi
             JOIN conversations c ON c.id = mi.conversation_id
             LEFT JOIN messages m ON m.id = (
               SELECT ms.message_id FROM memory_sources ms
               JOIN messages m2 ON m2.id = ms.message_id
               WHERE ms.memory_item_id = mi.id ORDER BY m2.created_at LIMIT 1)
             WHERE mi.status != 'dismissed'
             ORDER BY m.created_at",
        )?
        .query_map([], |r| {
            Ok(MemoryRow {
                id: r.get(0)?,
                kind: r.get(1)?,
                text: r.get(2)?,
                status: r.get(3)?,
                conversation_id: r.get(4)?,
                conversation_name: r.get(5)?,
                source_uuid: r.get(6)?,
                source_on_main_path: r.get::<_, Option<bool>>(7)?.unwrap_or(true),
                source_at: r.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// A user verdict; tagging re-runs never overwrite it.
pub fn set_memory_status(conn: &Connection, id: i64, status: MemoryStatus) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE memory_items SET status = ?1 WHERE id = ?2",
        (status.as_str(), id),
    )?;
    if n == 0 {
        return Err(AppError::InvalidInput("That item no longer exists.".into()));
    }
    Ok(())
}

/// A user edit: excluded chats are never sent to the API.
pub fn set_excluded(conn: &mut Connection, ids: &[i64], excluded: bool) -> AppResult<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare_cached("UPDATE conversations SET excluded = ?1 WHERE id = ?2")?;
        for id in ids {
            stmt.execute((excluded, id))?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaggingStats {
    pub tagged_conversations: i64,
    pub themes: i64,
    pub suggested_items: i64,
    pub total_cost_usd: f64,
}

pub fn tagging_stats(conn: &Connection) -> AppResult<TaggingStats> {
    use crate::pipeline::llm::Usage;
    let (tagged, themes, suggested) = conn.query_row(
        "SELECT (SELECT count(*) FROM tagged_conversations),
                (SELECT count(*) FROM themes WHERE merged_into IS NULL),
                (SELECT count(*) FROM memory_items WHERE status = 'suggested')",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let usage = conn
        .query_row(
            "SELECT sum(input_tokens), sum(output_tokens), sum(cache_read_tokens), sum(cache_write_tokens)
             FROM llm_runs",
            [],
            |r| {
                Ok(Usage {
                    input_tokens: r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    output_tokens: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    cache_read_tokens: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    cache_write_tokens: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                })
            },
        )
        .optional()?
        .unwrap_or_default();
    Ok(TaggingStats {
        tagged_conversations: tagged,
        themes,
        suggested_items: suggested,
        total_cost_usd: usage.cost_usd(),
    })
}
