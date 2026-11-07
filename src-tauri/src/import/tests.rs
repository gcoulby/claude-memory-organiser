//! Importer tests. All fixtures are synthetic: never put real export data here.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rusqlite::Connection;
use serde_json::{Value, json};
use zip::write::SimpleFileOptions;

use super::*;
use crate::db::{self, vault::DbKey};

fn msg(uuid: &str, parent: Option<&str>, sender: &str, text: &str, at: &str) -> Value {
    json!({
        "uuid": uuid,
        "parent_message_uuid": parent.unwrap_or(ROOT_PARENT),
        "sender": sender,
        // The flattened field carries UI placeholders the importer must ignore.
        "text": format!("{text}\n\nThis block is not supported on your current device yet."),
        "content": [
            { "type": "text", "text": text, "citations": [] },
            { "type": "tool_use", "name": "web_fetch", "input": { "url": "https://example.com" } }
        ],
        "created_at": at,
        "updated_at": at,
        "attachments": [],
        "files": []
    })
}

fn conv(uuid: &str, name: &str, updated: &str, messages: Vec<Value>) -> Value {
    json!({
        "uuid": uuid,
        "name": name,
        "summary": "",
        "created_at": "2026-01-01T10:00:00.000000Z",
        "updated_at": updated,
        "account": { "uuid": "acct-1" },
        "chat_messages": messages
    })
}

fn two_message_chat(uuid: &str) -> Value {
    conv(
        uuid,
        "Sourdough starter",
        "2026-01-01T10:05:00.000000Z",
        vec![
            msg(
                &format!("{uuid}-m1"),
                None,
                "human",
                "How do I feed a starter?",
                "2026-01-01T10:00:00.000000Z",
            ),
            msg(
                &format!("{uuid}-m2"),
                Some(&format!("{uuid}-m1")),
                "assistant",
                "Equal weights of flour and water.",
                "2026-01-01T10:01:00.000000Z",
            ),
        ],
    )
}

struct Fixture {
    dir: tempfile::TempDir,
    conn: Connection,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = db::open(
            &dir.path().join("memory.db"),
            &DbKey::for_tests(9),
            &dir.path().join("backups"),
        )
        .expect("open db");
        Self { dir, conn }
    }

    fn write_json(&self, name: &str, convs: &[Value]) -> PathBuf {
        let sub = self.dir.path().join(name);
        std::fs::create_dir_all(&sub).expect("mkdir");
        let path = sub.join("conversations.json");
        std::fs::write(&path, serde_json::to_vec(convs).expect("json")).expect("write");
        path
    }

    fn write_zip(&self, name: &str, entries: &[(&str, String)]) -> PathBuf {
        let path = self.dir.path().join(name);
        let mut zip = zip::ZipWriter::new(File::create(&path).expect("create"));
        for (entry, body) in entries {
            zip.start_file(*entry, SimpleFileOptions::default())
                .expect("start");
            zip.write_all(body.as_bytes()).expect("write");
        }
        zip.finish().expect("finish");
        path
    }

    fn import(&mut self, paths: &[PathBuf]) -> AppResult<ImportSummary> {
        run_import(&mut self.conn, paths, &AtomicBool::new(false), &mut |_| {})
    }

    fn count(&self, sql: &str) -> i64 {
        self.conn.query_row(sql, [], |r| r.get(0)).expect("count")
    }

    fn string(&self, sql: &str) -> String {
        self.conn.query_row(sql, [], |r| r.get(0)).expect("string")
    }
}

fn path_of(p: &Path) -> Vec<PathBuf> {
    vec![p.to_path_buf()]
}

#[test]
fn importing_the_same_export_twice_changes_nothing() {
    let mut f = Fixture::new();
    let file = f.write_json("a", &[two_message_chat("c1"), two_message_chat("c2")]);

    let first = f.import(&path_of(&file)).expect("first");
    assert_eq!(first.conversations_added, 2);
    assert_eq!(first.messages_added, 4);

    let second = f.import(&path_of(&file)).expect("second");
    assert_eq!(second.conversations_added, 0);
    assert_eq!(second.conversations_updated, 0);
    assert_eq!(second.conversations_unchanged, 2);
    assert_eq!(second.messages_added, 0);

    assert_eq!(f.count("SELECT count(*) FROM conversations"), 2);
    assert_eq!(f.count("SELECT count(*) FROM messages"), 4);
    assert_eq!(f.count("SELECT count(*) FROM imports"), 2);
}

#[test]
fn conversation_that_gained_messages_is_updated_and_user_state_survives() {
    let mut f = Fixture::new();
    let v1 = f.write_json("v1", &[two_message_chat("c1")]);
    f.import(&path_of(&v1)).expect("v1");

    // The user organises the chat between exports.
    f.conn
        .execute(
            "UPDATE conversations SET state = 'archived', excluded = 1",
            [],
        )
        .expect("user edit");

    let mut grown = two_message_chat("c1");
    grown["updated_at"] = json!("2026-02-01T09:00:00.000000Z");
    let msgs = grown["chat_messages"].as_array_mut().expect("array");
    msgs.push(msg(
        "c1-m3",
        Some("c1-m2"),
        "human",
        "And rye flour?",
        "2026-02-01T08:00:00.000000Z",
    ));
    msgs.push(msg(
        "c1-m4",
        Some("c1-m3"),
        "assistant",
        "Rye ferments faster.",
        "2026-02-01T08:01:00.000000Z",
    ));
    let v2 = f.write_json("v2", &[grown]);

    let summary = f.import(&path_of(&v2)).expect("v2");
    assert_eq!(summary.conversations_updated, 1);
    assert_eq!(summary.messages_added, 2);
    assert_eq!(summary.messages_updated, 0);

    assert_eq!(f.count("SELECT message_count FROM conversations"), 4);
    assert_eq!(f.string("SELECT state FROM conversations"), "archived");
    assert_eq!(f.count("SELECT excluded FROM conversations"), 1);
    assert_eq!(
        f.string("SELECT updated_at FROM conversations"),
        "2026-02-01T09:00:00.000000Z"
    );
}

#[test]
fn messages_missing_from_a_later_export_are_kept() {
    let mut f = Fixture::new();
    let v1 = f.write_json("v1", &[two_message_chat("c1")]);
    f.import(&path_of(&v1)).expect("v1");

    let mut shrunk = two_message_chat("c1");
    shrunk["chat_messages"].as_array_mut().expect("array").pop();
    shrunk["updated_at"] = json!("2026-03-01T00:00:00.000000Z");
    let v2 = f.write_json("v2", &[shrunk]);
    f.import(&path_of(&v2)).expect("v2");

    assert_eq!(f.count("SELECT count(*) FROM messages"), 2);
}

#[test]
fn unknown_fields_and_block_types_are_kept_raw() {
    let mut f = Fixture::new();
    let mut c = two_message_chat("c1");
    c["future_field"] = json!({ "nested": [1, 2, 3] });
    let m = &mut c["chat_messages"][0];
    m["new_message_field"] = json!("kept");
    m["content"]
        .as_array_mut()
        .expect("array")
        .push(json!({ "type": "hologram", "beam": "blue" }));
    let file = f.write_json("a", &[c]);
    f.import(&path_of(&file)).expect("import");

    let conv_raw: Value =
        serde_json::from_str(&f.string("SELECT raw_json FROM conversations")).expect("json");
    assert_eq!(conv_raw["future_field"]["nested"], json!([1, 2, 3]));
    assert!(conv_raw.get("chat_messages").is_none());

    let msg_raw: Value =
        serde_json::from_str(&f.string("SELECT raw_json FROM messages WHERE uuid = 'c1-m1'"))
            .expect("json");
    assert_eq!(msg_raw["new_message_field"], "kept");
    assert_eq!(msg_raw["content"][2]["type"], "hologram");
    // The unknown block does not leak into the text.
    assert_eq!(
        f.string("SELECT text FROM messages WHERE uuid = 'c1-m1'"),
        "How do I feed a starter?"
    );
}

#[test]
fn search_text_comes_from_content_blocks_not_the_flattened_field() {
    let mut f = Fixture::new();
    let file = f.write_json("a", &[two_message_chat("c1")]);
    f.import(&path_of(&file)).expect("import");

    assert_eq!(
        f.count("SELECT count(*) FROM messages_fts WHERE messages_fts MATCH 'flour'"),
        1
    );
    assert_eq!(
        f.count("SELECT count(*) FROM messages_fts WHERE messages_fts MATCH 'supported'"),
        0
    );
    // Prefix search, as used while typing.
    assert_eq!(
        f.count("SELECT count(*) FROM messages_fts WHERE messages_fts MATCH 'star*'"),
        1
    );
}

#[test]
fn attachments_are_searchable() {
    let mut f = Fixture::new();
    let mut c = two_message_chat("c1");
    c["chat_messages"][0]["attachments"] = json!([
        { "file_name": "notes.txt", "file_size": 10, "file_type": "txt", "extracted_content": "hydration percentages" }
    ]);
    let file = f.write_json("a", &[c]);
    f.import(&path_of(&file)).expect("import");
    assert_eq!(
        f.count("SELECT count(*) FROM messages_fts WHERE messages_fts MATCH 'hydration'"),
        1
    );
}

#[test]
fn main_path_follows_the_most_recent_branch() {
    let mut f = Fixture::new();
    // m1 -> m2 -> (m3a retried as m3b). m3b is newer, so it is on the main path.
    // Also an edited first prompt: a second root r2 that is older than m1's line.
    let c = conv(
        "c1",
        "Branchy",
        "2026-01-02T00:00:00.000000Z",
        vec![
            msg(
                "r2",
                None,
                "human",
                "first attempt",
                "2026-01-01T09:00:00.000000Z",
            ),
            msg("m1", None, "human", "hello", "2026-01-01T10:00:00.000000Z"),
            msg(
                "m2",
                Some("m1"),
                "assistant",
                "hi",
                "2026-01-01T10:01:00.000000Z",
            ),
            msg(
                "m3b",
                Some("m2"),
                "human",
                "second try",
                "2026-01-01T10:05:00.000000Z",
            ),
            msg(
                "m3a",
                Some("m2"),
                "human",
                "first try",
                "2026-01-01T10:02:00.000000Z",
            ),
        ],
    );
    let file = f.write_json("a", &[c]);
    f.import(&path_of(&file)).expect("import");

    let path: Vec<String> = f
        .conn
        .prepare("SELECT uuid FROM messages WHERE idx IS NOT NULL ORDER BY idx")
        .expect("prepare")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert_eq!(path, ["m1", "m2", "m3b"]);
    assert_eq!(
        f.count("SELECT count(*) FROM messages WHERE idx IS NULL"),
        2
    );
    assert_eq!(f.count("SELECT message_count FROM conversations"), 3);
    // The root sentinel is stored as NULL.
    assert_eq!(
        f.count("SELECT count(*) FROM messages WHERE parent_uuid IS NULL"),
        2
    );
}

#[test]
fn reads_split_zips_from_a_folder_and_skips_personal_files() {
    let mut f = Fixture::new();
    let convs = serde_json::to_string(&[two_message_chat("c1")]).expect("json");
    f.write_zip("conversations-000.zip", &[("conversations.json", convs)]);
    f.write_zip(
        "projects-000.zip",
        &[(
            "projects/p1.json",
            json!({ "uuid": "p1", "name": "Baking" }).to_string(),
        )],
    );
    f.write_zip(
        "light_metadata-000.zip",
        &[(
            "login_history.json",
            json!({ "login_events": [] }).to_string(),
        )],
    );
    // The fixture folder also holds the database; non-export files are ignored.
    let folder = f.dir.path().to_path_buf();

    let first = f.import(std::slice::from_ref(&folder)).expect("import");
    assert_eq!(first.conversations_added, 1);
    assert_eq!(first.files_stored, 1);
    assert_eq!(f.string("SELECT kind FROM export_files"), "project");
    assert_eq!(
        f.string("SELECT sources FROM imports"),
        r#"["conversations-000.zip","light_metadata-000.zip","projects-000.zip"]"#
    );

    let second = f.import(&[folder]).expect("reimport");
    assert_eq!(second.files_stored, 0);
    assert_eq!(f.count("SELECT count(*) FROM export_files"), 1);
}

#[test]
fn cancelling_leaves_the_database_untouched() {
    let mut f = Fixture::new();
    let file = f.write_json("a", &[two_message_chat("c1")]);
    let result = run_import(
        &mut f.conn,
        &path_of(&file),
        &AtomicBool::new(true),
        &mut |_| {},
    );
    assert!(matches!(result, Err(AppError::Cancelled)));
    assert_eq!(f.count("SELECT count(*) FROM conversations"), 0);
    assert_eq!(f.count("SELECT count(*) FROM imports"), 0);
}

#[test]
fn malformed_entries_are_skipped_not_fatal() {
    let mut f = Fixture::new();
    let file = f.write_json(
        "a",
        &[
            json!("not an object"),
            json!({ "name": "no uuid" }),
            two_message_chat("c1"),
        ],
    );
    let summary = f.import(&path_of(&file)).expect("import");
    assert_eq!(summary.conversations_skipped, 2);
    assert_eq!(summary.conversations_added, 1);
}

#[test]
fn rejects_files_that_are_not_exports() {
    let mut f = Fixture::new();
    let other = f.dir.path().join("notes.txt");
    std::fs::write(&other, "hello").expect("write");
    assert!(matches!(f.import(&[other]), Err(AppError::InvalidInput(_))));
}

/// Runs against a real export without committing or printing any of it:
/// `MO_EXPORT_DIR=../.temp/export cargo test --release real_export -- --ignored --nocapture`
#[test]
#[ignore = "needs a real export on disk"]
fn real_export_imports_cleanly_and_idempotently() {
    let Ok(dir) = std::env::var("MO_EXPORT_DIR") else {
        return;
    };
    let mut f = Fixture::new();
    let paths = vec![PathBuf::from(dir)];
    let first = f.import(&paths).expect("first import");
    println!("first:  {first:?}");
    let second = f.import(&paths).expect("second import");
    println!("second: {second:?}");
    assert_eq!(second.conversations_added + second.conversations_updated, 0);
    assert_eq!(second.messages_added + second.messages_updated, 0);

    let t = Instant::now();
    let hits = f.count("SELECT count(*) FROM messages_fts WHERE messages_fts MATCH 'th*'");
    println!("prefix search: {hits} hits in {:?}", t.elapsed());
    let size = std::fs::metadata(f.dir.path().join("memory.db"))
        .expect("meta")
        .len();
    println!("database size: {} MB", size / 1_000_000);
    for q in ["t", "th", "the", "code", "how do", "a b c d"] {
        let t = Instant::now();
        let r = crate::db::search::search(&f.conn, q, 200).expect("search");
        println!("search {q:?}: {} total in {:?}", r.total, t.elapsed());
    }
    let biggest = f.count("SELECT id FROM conversations ORDER BY message_count DESC LIMIT 1");
    let t = Instant::now();
    let detail = crate::db::queries::get_conversation(&f.conn, biggest, None).expect("detail");
    println!(
        "largest chat: {} messages read in {:?}",
        detail.messages.len(),
        t.elapsed()
    );
    let t = Instant::now();
    let all = crate::db::queries::list_conversations(&f.conn, None).expect("list");
    println!("list: {} chats in {:?}", all.len(), t.elapsed());
    println!(
        "main-path messages: {}, off-path: {}",
        f.count("SELECT count(*) FROM messages WHERE idx IS NOT NULL"),
        f.count("SELECT count(*) FROM messages WHERE idx IS NULL")
    );
}

fn branchy_chat() -> Value {
    conv(
        "c1",
        "Branchy",
        "2026-01-02T00:00:00.000000Z",
        vec![
            msg("m1", None, "human", "hello", "2026-01-01T10:00:00.000000Z"),
            msg(
                "m2",
                Some("m1"),
                "assistant",
                "hi",
                "2026-01-01T10:01:00.000000Z",
            ),
            msg(
                "m3a",
                Some("m2"),
                "human",
                "pumpernickel question",
                "2026-01-01T10:02:00.000000Z",
            ),
            msg(
                "m4a",
                Some("m3a"),
                "assistant",
                "an answer",
                "2026-01-01T10:03:00.000000Z",
            ),
            msg(
                "m3b",
                Some("m2"),
                "human",
                "second try",
                "2026-01-01T10:05:00.000000Z",
            ),
        ],
    )
}

#[test]
fn reader_shows_main_path_and_can_follow_another_branch() {
    use crate::db::queries::get_conversation;
    let mut f = Fixture::new();
    let file = f.write_json("a", &[branchy_chat()]);
    f.import(&path_of(&file)).expect("import");
    let id = f.count("SELECT id FROM conversations");

    let main = get_conversation(&f.conn, id, None).expect("main");
    let uuids: Vec<&str> = main.messages.iter().map(|m| m.uuid.as_str()).collect();
    assert_eq!(uuids, ["m1", "m2", "m3b"]);
    assert!(!main.off_main_path);
    let last = main.messages.last().expect("last");
    assert_eq!(last.siblings, ["m3a", "m3b"]);
    assert_eq!(last.sibling_index, 1);
    assert_eq!(main.messages[0].tools[0].name, "web_fetch");

    let other = get_conversation(&f.conn, id, Some("m3a")).expect("branch");
    let uuids: Vec<&str> = other.messages.iter().map(|m| m.uuid.as_str()).collect();
    assert_eq!(uuids, ["m1", "m2", "m3a", "m4a"]);
    assert!(other.off_main_path);
}

#[test]
fn triage_state_set_by_the_user_survives_reimport() {
    use crate::db::queries::{ChatState, list_conversations, set_state};
    let mut f = Fixture::new();
    let v1 = f.write_json("v1", &[two_message_chat("c1"), two_message_chat("c2")]);
    f.import(&path_of(&v1)).expect("v1");
    let id = f.count("SELECT id FROM conversations WHERE uuid = 'c1'");
    set_state(&mut f.conn, &[id], ChatState::Organised).expect("set");

    let mut changed = two_message_chat("c1");
    changed["name"] = json!("Renamed upstream");
    changed["updated_at"] = json!("2026-05-01T00:00:00.000000Z");
    let v2 = f.write_json("v2", &[changed, two_message_chat("c2")]);
    f.import(&path_of(&v2)).expect("v2");

    let organised = list_conversations(&f.conn, Some(ChatState::Organised)).expect("list");
    assert_eq!(organised.len(), 1);
    assert_eq!(organised[0].name, "Renamed upstream");
    assert_eq!(list_conversations(&f.conn, None).expect("all").len(), 2);
}

#[test]
fn search_finds_messages_on_any_branch_with_marked_snippets() {
    use crate::db::search::search;
    let mut f = Fixture::new();
    let file = f.write_json("a", &[branchy_chat()]);
    f.import(&path_of(&file)).expect("import");

    let results = search(&f.conn, "pumper", 10).expect("search");
    assert_eq!(results.total, 1);
    let hit = &results.hits[0];
    assert_eq!(hit.message_uuid, "m3a");
    assert!(!hit.on_main_path);
    assert!(hit.snippet.contains("\u{2}pumpernickel\u{3}"));
    assert_eq!(hit.conversation_name, "Branchy");
}
