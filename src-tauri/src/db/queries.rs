use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::tree::{Node, Tree};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatState {
    Untriaged,
    Organised,
    Archived,
}

impl ChatState {
    fn as_str(self) -> &'static str {
        match self {
            ChatState::Untriaged => "untriaged",
            ChatState::Organised => "organised",
            ChatState::Archived => "archived",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "organised" => ChatState::Organised,
            "archived" => ChatState::Archived,
            _ => ChatState::Untriaged,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRow {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub state: ChatState,
    pub message_count: i64,
}

/// All conversations, newest activity first. A few thousand rows is small
/// enough to send whole; the list is virtualised on the UI side.
pub fn list_conversations(
    conn: &Connection,
    state: Option<ChatState>,
) -> AppResult<Vec<ConversationRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, created_at, updated_at, state, message_count
         FROM conversations
         WHERE ?1 IS NULL OR state = ?1
         ORDER BY updated_at DESC, id DESC",
    )?;
    let rows = stmt
        .query_map([state.map(ChatState::as_str)], |r| {
            Ok(ConversationRow {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
                state: ChatState::parse(&r.get::<_, String>(4)?),
                message_count: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateCounts {
    pub untriaged: i64,
    pub organised: i64,
    pub archived: i64,
}

pub fn state_counts(conn: &Connection) -> AppResult<StateCounts> {
    let mut counts = StateCounts {
        untriaged: 0,
        organised: 0,
        archived: 0,
    };
    let mut stmt =
        conn.prepare_cached("SELECT state, count(*) FROM conversations GROUP BY state")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (state, n) = row?;
        match ChatState::parse(&state) {
            ChatState::Untriaged => counts.untriaged += n,
            ChatState::Organised => counts.organised += n,
            ChatState::Archived => counts.archived += n,
        }
    }
    Ok(counts)
}

/// A user edit: never touched by the importer.
pub fn set_state(conn: &mut Connection, ids: &[i64], state: ChatState) -> AppResult<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare_cached("UPDATE conversations SET state = ?1 WHERE id = ?2")?;
        for id in ids {
            stmt.execute((state.as_str(), id))?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUse {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderMessage {
    pub id: i64,
    pub uuid: String,
    pub sender: String,
    pub text: String,
    pub created_at: String,
    pub attachments: Vec<String>,
    pub files: Vec<String>,
    pub tools: Vec<ToolUse>,
    pub has_thinking: bool,
    /// Uuids of this message and its alternatives, oldest first.
    pub siblings: Vec<String>,
    pub sibling_index: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDetail {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub state: ChatState,
    /// True when showing a branch other than the main path.
    pub off_main_path: bool,
    pub messages: Vec<ReaderMessage>,
}

struct StoredMessage {
    id: i64,
    sender: String,
    text: String,
    created_at: String,
    on_main_path: bool,
    raw_json: String,
}

/// A conversation as the reader shows it: the main path, or the branch
/// running through the message `via` when given.
pub fn get_conversation(
    conn: &Connection,
    id: i64,
    via: Option<&str>,
) -> AppResult<ConversationDetail> {
    let header = conn
        .query_row(
            "SELECT uuid, name, created_at, updated_at, state FROM conversations WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::InvalidInput("That chat no longer exists.".into()))?;

    let mut stmt = conn.prepare_cached(
        "SELECT id, uuid, parent_uuid, created_at, array_idx, sender, text, idx IS NOT NULL, raw_json
         FROM messages WHERE conversation_id = ?1",
    )?;
    let (nodes, stored): (Vec<Node>, Vec<StoredMessage>) = stmt
        .query_map([id], |r| {
            Ok((
                Node {
                    uuid: r.get(1)?,
                    parent: r.get(2)?,
                    created_at: r.get(3)?,
                    array_idx: r.get(4)?,
                },
                StoredMessage {
                    id: r.get(0)?,
                    created_at: r.get(3)?,
                    sender: r.get(5)?,
                    text: r.get(6)?,
                    on_main_path: r.get(7)?,
                    raw_json: r.get(8)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .unzip();

    let tree = Tree::new(&nodes);
    // With no `via` this recomputes the main path the importer stored,
    // using the same rule, so the two always agree.
    let path = tree.path_through(via.and_then(|v| tree.index_of(v)));
    let off_main_path = path.iter().any(|&i| !stored[i].on_main_path);

    let messages = path
        .iter()
        .map(|&i| {
            let s = &stored[i];
            let raw: Value = serde_json::from_str(&s.raw_json).unwrap_or(Value::Null);
            let siblings = tree.siblings(i);
            ReaderMessage {
                id: s.id,
                uuid: nodes[i].uuid.clone(),
                sender: s.sender.clone(),
                text: s.text.clone(),
                created_at: s.created_at.clone(),
                attachments: names(&raw, "attachments"),
                files: names(&raw, "files"),
                tools: tools_used(&raw),
                has_thinking: has_block(&raw, "thinking"),
                siblings: siblings.iter().map(|&j| nodes[j].uuid.clone()).collect(),
                sibling_index: siblings.iter().position(|&j| j == i).unwrap_or(0),
            }
        })
        .collect();

    let (uuid, name, created_at, updated_at, state) = header;
    Ok(ConversationDetail {
        id,
        uuid,
        name,
        created_at,
        updated_at,
        state: ChatState::parse(&state),
        off_main_path,
        messages,
    })
}

fn blocks(raw: &Value) -> impl Iterator<Item = &Value> {
    raw.get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn has_block(raw: &Value, kind: &str) -> bool {
    blocks(raw).any(|b| b.get("type").and_then(Value::as_str) == Some(kind))
}

fn names(raw: &Value, key: &str) -> Vec<String> {
    raw.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| a.get("file_name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// Tool calls in a message, grouped by display name in first-use order.
fn tools_used(raw: &Value) -> Vec<ToolUse> {
    let mut out: Vec<ToolUse> = Vec::new();
    for b in blocks(raw).filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use")) {
        let name = b
            .get("integration_name")
            .and_then(Value::as_str)
            .or_else(|| b.get("name").and_then(Value::as_str))
            .unwrap_or("tool");
        match out.iter_mut().find(|t| t.name == name) {
            Some(t) => t.count += 1,
            None => out.push(ToolUse {
                name: name.to_owned(),
                count: 1,
            }),
        }
    }
    out
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationEdit {
    pub uuid: String,
    pub name: String,
    pub state: ChatState,
    pub excluded: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditsExport {
    pub format: u32,
    pub app_version: &'static str,
    pub exported_at: i64,
    /// Chats whose state or exclusion the user has changed.
    pub conversations: Vec<ConversationEdit>,
}

/// Everything the user has decided, as opposed to anything derivable from
/// the export. Grows as themes and memory items arrive.
pub fn export_edits(conn: &Connection) -> AppResult<EditsExport> {
    let mut stmt = conn.prepare(
        "SELECT uuid, name, state, excluded FROM conversations
         WHERE state != 'untriaged' OR excluded = 1
         ORDER BY updated_at DESC",
    )?;
    let conversations = stmt
        .query_map([], |r| {
            Ok(ConversationEdit {
                uuid: r.get(0)?,
                name: r.get(1)?,
                state: ChatState::parse(&r.get::<_, String>(2)?),
                excluded: r.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(EditsExport {
        format: 1,
        app_version: env!("CARGO_PKG_VERSION"),
        exported_at: crate::db::now_ms(),
        conversations,
    })
}
