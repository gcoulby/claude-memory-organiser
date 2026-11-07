//! Segmenting a conversation, tagging its segments and extracting memory
//! items, in one model call per conversation.
//!
//! How user edits survive a re-run (CLAUDE.md, principle 1):
//! - Segmentation is replaced only while it is untouched. Once the user has
//!   added a segment, or put a theme or tag on one by hand, the conversation's
//!   segments are left alone.
//! - Only `suggested` memory items are replaced. Confirmed and dismissed
//!   items stay, and a suggestion matching a dismissed item is not re-added.
//! - Theme names resolve through merges, so a merged theme stays merged.

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::db::now_ms;
use crate::error::{AppError, AppResult};

/// Bump when the prompt or schema changes, so cached results are not reused.
pub const TASK: &str = "segment_tag_v1";
const MAX_THEMES_PER_SEGMENT: usize = 3;

pub const INTENTS: &[&str] = &[
    "debugging",
    "building",
    "planning",
    "research",
    "learning",
    "writing",
    "decision",
    "brainstorming",
    "reflection",
    "personal",
    "admin",
    "other",
];

pub const MEMORY_KINDS: &[&str] = &["decision", "preference", "open_question", "project", "fact"];

pub const SYSTEM_PROMPT: &str = "\
You organise one person's chat history with an AI assistant. You receive one chat, \
with each message numbered on the left. Return JSON matching the schema.

Segments
- Split the chat into segments wherever the topic genuinely changes. Most chats are \
one segment; split only when a later part is about something unrelated.
- Segments must cover every message, in order, without gaps or overlaps: the first \
starts at 0, each next one starts right after the previous ends, the last ends at \
the final message.
- title: a short, specific label for what the segment is about (under 8 words).
- summary: one or two plain sentences on what was asked and what came of it.
- intent: what the person was doing.
- themes: 1 to 3 broad topics the segment belongs to, the kind of heading someone \
would file it under (for example \"Home server\", \"Job search\", \"Godot game dev\"). \
Prefer a theme from the existing list when one fits, spelled exactly as listed. \
Create a new theme only when none fits. confidence is 0 to 1.
- entities: specific named things discussed (tools, products, libraries, places, \
projects). At most 6, no generic words.

Memory items: things worth keeping from this chat, each tied to the messages it \
comes from.
- decision: something the person decided or settled on.
- preference: a stated like, dislike or way they want things done.
- open_question: a question or problem the person raised that the chat did not \
resolve, or a plan they started and did not finish.
- project: an ongoing piece of work they are doing.
- fact: a durable fact about the person or their situation.
Write each as a standalone sentence that makes sense without the chat. Include \
only items that matter beyond this one chat; an empty list is fine. Never invent \
anything not in the messages.";

pub fn schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["segments", "memory_items"],
        "properties": {
            "segments": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["start_index", "end_index", "title", "summary", "intent", "themes", "entities"],
                    "properties": {
                        "start_index": { "type": "integer" },
                        "end_index": { "type": "integer" },
                        "title": { "type": "string" },
                        "summary": { "type": "string" },
                        "intent": { "type": "string", "enum": INTENTS },
                        "themes": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "required": ["name", "confidence"],
                                "properties": {
                                    "name": { "type": "string" },
                                    "confidence": { "type": "number" }
                                }
                            }
                        },
                        "entities": { "type": "array", "items": { "type": "string" } }
                    }
                }
            },
            "memory_items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["kind", "text", "message_indices"],
                    "properties": {
                        "kind": { "type": "string", "enum": MEMORY_KINDS },
                        "text": { "type": "string" },
                        "message_indices": { "type": "array", "items": { "type": "integer" } }
                    }
                }
            }
        }
    })
}

/// What gets sent for one conversation.
pub struct TagInput {
    pub conversation_id: i64,
    /// Message ids in main-path order; model indices point into this.
    pub message_ids: Vec<i64>,
    pub transcript: String,
    pub hash: String,
}

/// Builds the transcript of a conversation's main path.
pub fn build_input(conn: &Connection, conversation_id: i64) -> AppResult<TagInput> {
    let name: String = conn.query_row(
        "SELECT name FROM conversations WHERE id = ?1",
        [conversation_id],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare_cached(
        "SELECT id, sender, created_at, text, attachment_text FROM messages
         WHERE conversation_id = ?1 AND idx IS NOT NULL ORDER BY idx",
    )?;
    let rows = stmt.query_map([conversation_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;

    let mut transcript = format!(
        "Chat title: {}\n\n",
        if name.is_empty() { "(untitled)" } else { &name }
    );
    let mut message_ids = Vec::new();
    for row in rows {
        let (id, sender, at, text, attachment) = row?;
        let who = if sender == "human" {
            "PERSON"
        } else {
            "ASSISTANT"
        };
        let date = at.get(..10).unwrap_or(&at);
        transcript.push_str(&format!("[{}] {who} ({date}):\n", message_ids.len()));
        transcript.push_str(if text.trim().is_empty() {
            "(no text)"
        } else {
            text.trim()
        });
        if !attachment.trim().is_empty() {
            transcript.push_str("\n<attached>\n");
            transcript.push_str(attachment.trim());
            transcript.push_str("\n</attached>");
        }
        transcript.push_str("\n\n");
        message_ids.push(id);
    }

    let mut hasher = Sha256::new();
    hasher.update(TASK.as_bytes());
    hasher.update(transcript.as_bytes());
    Ok(TagInput {
        conversation_id,
        message_ids,
        transcript,
        hash: hex::encode(hasher.finalize()),
    })
}

/// The user message: the current theme list (so names get reused), then the chat.
pub fn user_message(input: &TagInput, existing_themes: &[String]) -> String {
    let themes = if existing_themes.is_empty() {
        "(none yet)".to_owned()
    } else {
        existing_themes.join("\n")
    };
    format!(
        "Existing themes:\n{themes}\n\nThe chat has {} messages, numbered 0 to {}.\n\n{}",
        input.message_ids.len(),
        input.message_ids.len().saturating_sub(1),
        input.transcript
    )
}

#[derive(Debug, Deserialize)]
struct RawOutput {
    segments: Vec<RawSegment>,
    memory_items: Vec<RawMemory>,
}

#[derive(Debug, Deserialize)]
struct RawSegment {
    start_index: i64,
    end_index: i64,
    title: String,
    summary: String,
    intent: String,
    themes: Vec<RawTheme>,
    entities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawTheme {
    name: String,
    confidence: f64,
}

#[derive(Debug, Deserialize)]
struct RawMemory {
    kind: String,
    text: String,
    message_indices: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start: usize,
    pub end: usize,
    pub title: String,
    pub summary: String,
    pub intent: String,
    pub themes: Vec<(String, f64)>,
    pub entities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryItem {
    pub kind: String,
    pub text: String,
    pub message_indices: Vec<usize>,
}

#[derive(Debug, PartialEq)]
pub struct TagResult {
    pub segments: Vec<Segment>,
    pub memory_items: Vec<MemoryItem>,
    /// What had to be fixed, for judging output quality honestly.
    pub repairs: Vec<String>,
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Checks and repairs model output against a chat of `n` messages.
///
/// The schema guarantees shape, not sense: segments may overlap, leave gaps
/// or point past the end. Small problems are repaired and recorded rather
/// than discarding a paid-for answer.
pub fn validate(json: &Value, n: usize) -> AppResult<TagResult> {
    let raw: RawOutput = serde_json::from_value(json.clone())
        .map_err(|e| AppError::BadModelOutput(format!("unexpected shape ({e})")))?;
    let mut repairs = Vec::new();
    if n == 0 {
        return Ok(TagResult {
            segments: Vec::new(),
            memory_items: Vec::new(),
            repairs,
        });
    }
    let last = n - 1;
    let clamp = |i: i64| usize::try_from(i.max(0)).unwrap_or(0).min(last);

    let mut segs: Vec<RawSegment> = raw.segments;
    segs.sort_by_key(|s| s.start_index);
    let mut segments: Vec<Segment> = Vec::new();
    for s in segs {
        let mut start = clamp(s.start_index);
        let end = clamp(s.end_index);
        let expected = segments.last().map_or(0, |p: &Segment| p.end + 1);
        if start != expected {
            repairs.push(format!("segment start {start} moved to {expected}"));
            start = expected;
        }
        if start > last || end < start {
            repairs.push("dropped an empty or overlapping segment".into());
            continue;
        }

        let mut seen = HashSet::new();
        let mut themes: Vec<(String, f64)> = s
            .themes
            .iter()
            .map(|t| (clean(&t.name), t.confidence.clamp(0.0, 1.0)))
            .filter(|(name, _)| !name.is_empty() && seen.insert(name.to_lowercase()))
            .collect();
        themes.sort_by(|a, b| b.1.total_cmp(&a.1));
        if themes.len() > MAX_THEMES_PER_SEGMENT {
            repairs.push(format!(
                "kept the top {MAX_THEMES_PER_SEGMENT} of {} themes",
                themes.len()
            ));
            themes.truncate(MAX_THEMES_PER_SEGMENT);
        }
        let mut seen = HashSet::new();
        let entities = s
            .entities
            .iter()
            .map(|e| clean(e))
            .filter(|e| !e.is_empty() && seen.insert(e.to_lowercase()))
            .take(6)
            .collect();
        let intent = if INTENTS.contains(&s.intent.as_str()) {
            s.intent
        } else {
            "other".into()
        };
        segments.push(Segment {
            start,
            end,
            title: clean(&s.title),
            summary: s.summary.trim().to_owned(),
            intent,
            themes,
            entities,
        });
    }
    match segments.last_mut() {
        Some(seg) if seg.end != last => {
            repairs.push(format!("last segment extended from {} to {last}", seg.end));
            seg.end = last;
        }
        None => {
            return Err(AppError::BadModelOutput("no segments returned".into()));
        }
        _ => {}
    }

    let mut memory_items = Vec::new();
    for m in raw.memory_items {
        let text = m.text.trim().to_owned();
        let mut indices: Vec<usize> = m
            .message_indices
            .iter()
            .filter(|&&i| i >= 0 && (i as usize) <= last)
            .map(|&i| i as usize)
            .collect();
        indices.sort_unstable();
        indices.dedup();
        if text.is_empty() || indices.is_empty() || !MEMORY_KINDS.contains(&m.kind.as_str()) {
            repairs.push("dropped a memory item with no text or valid source".into());
            continue;
        }
        memory_items.push(MemoryItem {
            kind: m.kind,
            text,
            message_indices: indices,
        });
    }

    Ok(TagResult {
        segments,
        memory_items,
        repairs,
    })
}

/// Finds or creates a theme by name, following merges to the surviving theme.
fn resolve_theme(tx: &Transaction, name: &str) -> AppResult<i64> {
    let found: Option<(i64, Option<i64>)> = tx
        .prepare_cached("SELECT id, merged_into FROM themes WHERE name = ?1 COLLATE NOCASE")?
        .query_row([name], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let Some((mut id, mut merged)) = found else {
        tx.prepare_cached("INSERT INTO themes (name, origin, created_at) VALUES (?1, 'auto', ?2)")?
            .execute((name, now_ms()))?;
        return Ok(tx.last_insert_rowid());
    };
    // Bounded, in case of a merge cycle in damaged data.
    for _ in 0..32 {
        let Some(next) = merged else { break };
        id = next;
        merged = tx
            .prepare_cached("SELECT merged_into FROM themes WHERE id = ?1")?
            .query_row([id], |r| r.get(0))?;
    }
    Ok(id)
}

fn resolve_tag(tx: &Transaction, name: &str) -> AppResult<i64> {
    tx.prepare_cached("INSERT OR IGNORE INTO tags (name, origin) VALUES (?1, 'auto')")?
        .execute([name])?;
    Ok(tx
        .prepare_cached("SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE")?
        .query_row([name], |r| r.get(0))?)
}

/// True once the user has shaped this conversation's segments by hand.
fn segmentation_is_users(tx: &Transaction, conversation_id: i64) -> AppResult<bool> {
    let n: i64 = tx.query_row(
        "SELECT
           (SELECT count(*) FROM segments WHERE conversation_id = ?1 AND source = 'user')
         + (SELECT count(*) FROM segment_themes st JOIN segments s ON s.id = st.segment_id
            WHERE s.conversation_id = ?1 AND st.origin = 'user')
         + (SELECT count(*) FROM segment_tags st JOIN segments s ON s.id = st.segment_id
            WHERE s.conversation_id = ?1 AND st.origin = 'user')",
        [conversation_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Writes one conversation's result, replacing only what the pipeline owns.
pub fn store(
    tx: &Transaction,
    input: &TagInput,
    result: &TagResult,
    model: &str,
    run_id: Option<i64>,
) -> AppResult<()> {
    let conv = input.conversation_id;
    let keep_user_segments = segmentation_is_users(tx, conv)?;

    let mut segment_ids: Vec<(usize, usize, i64)> = Vec::new();
    if keep_user_segments {
        let mut stmt = tx.prepare_cached(
            "SELECT start_idx, end_idx, id FROM segments WHERE conversation_id = ?1 ORDER BY start_idx",
        )?;
        let rows = stmt.query_map([conv], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (s, e, id) = row?;
            segment_ids.push((
                usize::try_from(s).unwrap_or(0),
                usize::try_from(e).unwrap_or(0),
                id,
            ));
        }
    } else {
        // Memory items keep their place via ON DELETE SET NULL.
        tx.prepare_cached("DELETE FROM segments WHERE conversation_id = ?1 AND source = 'auto'")?
            .execute([conv])?;
        for seg in &result.segments {
            tx.prepare_cached(
                "INSERT INTO segments (conversation_id, start_idx, end_idx, title, summary, intent, source, run_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'auto', ?7)",
            )?
            .execute((
                conv,
                i64::try_from(seg.start).unwrap_or(0),
                i64::try_from(seg.end).unwrap_or(0),
                &seg.title,
                &seg.summary,
                &seg.intent,
                run_id,
            ))?;
            let seg_id = tx.last_insert_rowid();
            segment_ids.push((seg.start, seg.end, seg_id));
            for (name, confidence) in &seg.themes {
                let theme = resolve_theme(tx, name)?;
                tx.prepare_cached(
                    "INSERT OR IGNORE INTO segment_themes (segment_id, theme_id, origin, confidence)
                     VALUES (?1, ?2, 'auto', ?3)",
                )?
                .execute((seg_id, theme, confidence))?;
            }
            for entity in &seg.entities {
                let tag = resolve_tag(tx, entity)?;
                tx.prepare_cached(
                    "INSERT OR IGNORE INTO segment_tags (segment_id, tag_id, origin) VALUES (?1, ?2, 'auto')",
                )?
                .execute((seg_id, tag))?;
            }
        }
    }

    tx.prepare_cached(
        "DELETE FROM memory_items WHERE conversation_id = ?1 AND status = 'suggested' AND origin = 'auto'",
    )?
    .execute([conv])?;
    let decided: HashSet<String> = tx
        .prepare_cached(
            "SELECT lower(text) FROM memory_items WHERE conversation_id = ?1 AND status != 'suggested'",
        )?
        .query_map([conv], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for item in &result.memory_items {
        if decided.contains(&item.text.to_lowercase()) {
            continue;
        }
        let first = item.message_indices[0];
        let segment = segment_ids
            .iter()
            .find(|(s, e, _)| (*s..=*e).contains(&first))
            .map(|(_, _, id)| *id);
        tx.prepare_cached(
            "INSERT INTO memory_items (conversation_id, segment_id, kind, text, status, origin, created_at, run_id)
             VALUES (?1, ?2, ?3, ?4, 'suggested', 'auto', ?5, ?6)",
        )?
        .execute((conv, segment, &item.kind, &item.text, now_ms(), run_id))?;
        let item_id = tx.last_insert_rowid();
        for &i in &item.message_indices {
            if let Some(&message_id) = input.message_ids.get(i) {
                tx.prepare_cached(
                    "INSERT OR IGNORE INTO memory_sources (memory_item_id, message_id) VALUES (?1, ?2)",
                )?
                .execute((item_id, message_id))?;
            }
        }
    }

    tx.prepare_cached(
        "INSERT INTO tagged_conversations (conversation_id, content_hash, model, tagged_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (conversation_id) DO UPDATE SET
           content_hash = excluded.content_hash, model = excluded.model, tagged_at = excluded.tagged_at",
    )?
    .execute((conv, &input.hash, model, now_ms()))?;
    Ok(())
}

/// Names to offer the model for reuse: every live (unmerged) theme.
pub fn theme_vocabulary(conn: &Connection) -> AppResult<Vec<String>> {
    let names = conn
        .prepare_cached(
            "SELECT name FROM themes WHERE merged_into IS NULL ORDER BY name COLLATE NOCASE",
        )?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(names)
}
