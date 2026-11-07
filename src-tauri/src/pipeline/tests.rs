//! Pipeline tests with a fake model. All data is synthetic.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rusqlite::Connection;
use serde_json::{Value, json};

use super::llm::{Llm, LlmRequest, LlmResponse, Usage};
use super::tag::{self, validate};
use super::*;
use crate::db::{self, vault::DbKey};
use crate::error::{AppError, AppResult};

/// Answers every chat with one segment covering all messages.
struct FakeLlm {
    calls: AtomicUsize,
    themes: Vec<&'static str>,
    fail_with: Option<u16>,
}

impl FakeLlm {
    fn new(themes: Vec<&'static str>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            themes,
            fail_with: None,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Llm for FakeLlm {
    fn model(&self) -> &str {
        "fake-model"
    }

    fn complete(&self, req: &LlmRequest) -> AppResult<LlmResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(status) = self.fail_with {
            return Err(AppError::Api {
                status,
                message: "invalid x-api-key".into(),
            });
        }
        // Read the message count the prompt states, as the real model would.
        let n: i64 = req
            .user
            .split("The chat has ")
            .nth(1)
            .and_then(|r| r.split(' ').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(1);
        let themes: Vec<Value> = self
            .themes
            .iter()
            .map(|t| json!({ "name": t, "confidence": 0.9 }))
            .collect();
        Ok(LlmResponse {
            json: json!({
                "segments": [{
                    "start_index": 0, "end_index": n - 1,
                    "title": "Feeding a sourdough starter", "summary": "How often to feed.",
                    "intent": "learning", "themes": themes, "entities": ["Sourdough"]
                }],
                "memory_items": [
                    { "kind": "decision", "text": "Feed the starter twice a day.", "message_indices": [1] },
                    { "kind": "open_question", "text": "Does rye flour speed it up?", "message_indices": [0] }
                ]
            }),
            usage: Usage {
                input_tokens: 1000,
                output_tokens: 200,
                ..Usage::default()
            },
        })
    }
}

fn setup() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = db::open(
        &dir.path().join("m.db"),
        &DbKey::for_tests(3),
        &dir.path().join("b"),
    )
    .expect("open");
    conn.execute(
        "INSERT INTO imports (id, started_at, sources) VALUES (1, 0, '[]')",
        [],
    )
    .expect("import row");
    (dir, conn)
}

/// Adds a conversation with `n` messages on its main path.
fn add_chat(conn: &Connection, uuid: &str, n: usize) -> i64 {
    conn.execute(
        "INSERT INTO conversations (uuid, name, created_at, updated_at, message_count, raw_hash, raw_json, first_import_id, last_import_id)
         VALUES (?1, 'Bread', '2026-01-01T00:00:00Z', ?2, ?3, 'h', '{}', 1, 1)",
        (uuid, format!("2026-01-0{}T00:00:00Z", 1 + n % 9), n as i64),
    )
    .expect("conv");
    let id = conn.last_insert_rowid();
    for i in 0..n {
        add_message(conn, id, uuid, i);
    }
    id
}

fn add_message(conn: &Connection, conv: i64, uuid: &str, i: usize) {
    conn.execute(
        "INSERT INTO messages (conversation_id, uuid, sender, text, created_at, array_idx, idx, raw_json)
         VALUES (?1, ?2, ?3, ?4, '2026-01-01T00:00:00Z', ?5, ?5, '{}')",
        (
            conv,
            format!("{uuid}-m{i}"),
            if i.is_multiple_of(2) { "human" } else { "assistant" },
            format!("message {i} about starters"),
            i as i64,
        ),
    )
    .expect("message");
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).expect("count")
}

fn run_all(conn: &mut Connection, llm: &dyn Llm) -> AppResult<TagSummary> {
    let ids = select(conn, Scope::All)?;
    run(conn, llm, &ids, &AtomicBool::new(false), &mut |_| {})
}

#[test]
fn tags_chats_and_records_cost() {
    let (_d, mut conn) = setup();
    add_chat(&conn, "c1", 4);
    add_chat(&conn, "c2", 2);
    let llm = FakeLlm::new(vec!["Baking", "Home cooking"]);
    let summary = run_all(&mut conn, &llm).expect("run");

    assert_eq!(summary.tagged, 2);
    assert_eq!(llm.calls(), 2);
    assert_eq!(count(&conn, "SELECT count(*) FROM segments"), 2);
    assert_eq!(count(&conn, "SELECT count(*) FROM themes"), 2);
    assert_eq!(
        count(
            &conn,
            "SELECT count(*) FROM memory_items WHERE status = 'suggested'"
        ),
        4
    );
    assert_eq!(count(&conn, "SELECT count(*) FROM memory_sources"), 4);
    assert_eq!(count(&conn, "SELECT input_tokens FROM llm_runs"), 2000);
    assert_eq!(
        count(
            &conn,
            "SELECT count(*) FROM llm_runs WHERE status = 'completed'"
        ),
        1
    );
    assert!(summary.cost_usd > 0.0);
}

#[test]
fn rerun_skips_unchanged_chats_without_calling_the_api() {
    let (_d, mut conn) = setup();
    add_chat(&conn, "c1", 4);
    let llm = FakeLlm::new(vec!["Baking"]);
    run_all(&mut conn, &llm).expect("first");
    let second = run_all(&mut conn, &llm).expect("second");
    assert_eq!(second.skipped, 1);
    assert_eq!(llm.calls(), 1);
}

#[test]
fn cached_answers_are_reused() {
    let (_d, mut conn) = setup();
    add_chat(&conn, "c1", 4);
    let llm = FakeLlm::new(vec!["Baking"]);
    run_all(&mut conn, &llm).expect("first");
    conn.execute("DELETE FROM tagged_conversations", [])
        .expect("forget");
    let second = run_all(&mut conn, &llm).expect("second");
    assert_eq!(second.from_cache, 1);
    assert_eq!(llm.calls(), 1);
}

#[test]
fn user_decisions_survive_a_rerun_after_new_messages() {
    let (_d, mut conn) = setup();
    let id = add_chat(&conn, "c1", 4);
    let llm = FakeLlm::new(vec!["Baking"]);
    run_all(&mut conn, &llm).expect("first");

    // The user confirms one item, dismisses another and files the segment
    // under a theme of their own.
    conn.execute(
        "UPDATE memory_items SET status = 'confirmed' WHERE kind = 'decision'",
        [],
    )
    .expect("confirm");
    conn.execute(
        "UPDATE memory_items SET status = 'dismissed' WHERE kind = 'open_question'",
        [],
    )
    .expect("dismiss");
    conn.execute(
        "INSERT INTO themes (name, origin, created_at) VALUES ('Fermentation', 'user', 0)",
        [],
    )
    .expect("theme");
    conn.execute(
        "INSERT INTO segment_themes (segment_id, theme_id, origin)
         SELECT s.id, t.id, 'user' FROM segments s, themes t WHERE t.name = 'Fermentation'",
        [],
    )
    .expect("link");
    let segment_before = count(&conn, "SELECT id FROM segments");

    // The chat grows in a later export, so it is re-tagged.
    add_message(&conn, id, "c1", 4);
    add_message(&conn, id, "c1", 5);
    let second = run_all(&mut conn, &llm).expect("second");
    assert_eq!(second.tagged, 1);

    // The hand-edited segmentation is kept as it was.
    assert_eq!(count(&conn, "SELECT id FROM segments"), segment_before);
    assert_eq!(
        count(
            &conn,
            "SELECT count(*) FROM segment_themes WHERE origin = 'user'"
        ),
        1
    );
    // Confirmed stays, dismissed is not suggested again, nothing is duplicated.
    assert_eq!(
        count(
            &conn,
            "SELECT count(*) FROM memory_items WHERE status = 'confirmed'"
        ),
        1
    );
    assert_eq!(
        count(
            &conn,
            "SELECT count(*) FROM memory_items WHERE status = 'dismissed'"
        ),
        1
    );
    assert_eq!(
        count(
            &conn,
            "SELECT count(*) FROM memory_items WHERE status = 'suggested'"
        ),
        0
    );
}

#[test]
fn untouched_segmentation_is_replaced_on_rerun() {
    let (_d, mut conn) = setup();
    let id = add_chat(&conn, "c1", 4);
    let llm = FakeLlm::new(vec!["Baking"]);
    run_all(&mut conn, &llm).expect("first");
    add_message(&conn, id, "c1", 4);
    run_all(&mut conn, &llm).expect("second");
    assert_eq!(count(&conn, "SELECT count(*) FROM segments"), 1);
    assert_eq!(count(&conn, "SELECT end_idx FROM segments"), 4);
}

#[test]
fn merged_themes_stay_merged() {
    let (_d, mut conn) = setup();
    conn.execute_batch(
        "INSERT INTO themes (id, name, origin, created_at) VALUES (1, 'Cooking', 'user', 0);
         INSERT INTO themes (id, name, origin, merged_into, created_at) VALUES (2, 'Baking', 'auto', 1, 0);",
    )
    .expect("themes");
    add_chat(&conn, "c1", 2);
    let llm = FakeLlm::new(vec!["baking"]);
    run_all(&mut conn, &llm).expect("run");
    assert_eq!(count(&conn, "SELECT theme_id FROM segment_themes"), 1);
    assert_eq!(count(&conn, "SELECT count(*) FROM themes"), 2);
}

#[test]
fn excluded_chats_are_never_selected() {
    let (_d, conn) = setup();
    let keep = add_chat(&conn, "c1", 4);
    let skip = add_chat(&conn, "c2", 4);
    conn.execute(
        "UPDATE conversations SET excluded = 1 WHERE id = ?1",
        [skip],
    )
    .expect("exclude");
    assert_eq!(select(&conn, Scope::All).expect("all"), vec![keep]);
    assert_eq!(select(&conn, Scope::Sample).expect("sample"), vec![keep]);
}

#[test]
fn a_bad_key_stops_the_run() {
    let (_d, mut conn) = setup();
    for i in 0..10 {
        add_chat(&conn, &format!("c{i}"), 2);
    }
    let mut llm = FakeLlm::new(vec!["Baking"]);
    llm.fail_with = Some(401);
    let result = run_all(&mut conn, &llm);
    assert!(matches!(result, Err(AppError::Api { status: 401, .. })));
    // Workers stop soon after the first failure rather than trying them all.
    assert!(llm.calls() < 10, "made {} calls", llm.calls());
    assert_eq!(
        count(
            &conn,
            "SELECT count(*) FROM llm_runs WHERE status = 'failed'"
        ),
        1
    );
}

#[test]
fn estimate_counts_what_would_be_sent() {
    let (_d, mut conn) = setup();
    add_chat(&conn, "c1", 4);
    add_chat(&conn, "c2", 4);
    let ids = select(&conn, Scope::All).expect("ids");
    let before = estimate(&conn, &ids).expect("estimate");
    assert_eq!(before.to_send, 2);
    assert!(before.cost_usd > 0.0);
    // Results from another model do not count as done for the real one.
    run_all(&mut conn, &FakeLlm::new(vec!["Baking"])).expect("run");
    let after = estimate(&conn, &ids).expect("estimate");
    assert_eq!(after.to_send, 2);
}

#[test]
fn validate_repairs_gaps_overlaps_and_bad_indices() {
    let out = json!({
        "segments": [
            { "start_index": 5, "end_index": 9, "title": " Second  part ", "summary": "",
              "intent": "planning", "entities": [],
              "themes": [
                { "name": "A", "confidence": 0.2 }, { "name": "B", "confidence": 0.9 },
                { "name": "a", "confidence": 0.5 }, { "name": "C", "confidence": 1.5 },
                { "name": "D", "confidence": 0.1 }
              ] },
            { "start_index": 0, "end_index": 3, "title": "First", "summary": "",
              "intent": "debugging", "themes": [], "entities": ["x", "X", " "] }
        ],
        "memory_items": [
            { "kind": "fact", "text": "Lives in Leeds.", "message_indices": [2, 99, -1, 2] },
            { "kind": "fact", "text": "Nothing valid.", "message_indices": [42] }
        ]
    });
    let r = validate(&out, 8).expect("valid");
    assert_eq!(r.segments.len(), 2);
    assert_eq!((r.segments[0].start, r.segments[0].end), (0, 3));
    // The gap 4 is closed and the end clamped to the last message.
    assert_eq!((r.segments[1].start, r.segments[1].end), (4, 7));
    assert_eq!(r.segments[1].title, "Second part");
    let names: Vec<&str> = r.segments[1]
        .themes
        .iter()
        .map(|(n, _)| n.as_str())
        .collect();
    assert_eq!(names, ["C", "B", "A"]);
    assert_eq!(r.segments[1].themes[0].1, 1.0);
    assert_eq!(r.segments[0].entities, ["x"]);
    assert_eq!(r.memory_items.len(), 1);
    assert_eq!(r.memory_items[0].message_indices, [2]);
    assert!(!r.repairs.is_empty());
}

#[test]
fn validate_rejects_empty_segmentation() {
    let out = json!({ "segments": [], "memory_items": [] });
    assert!(matches!(
        validate(&out, 3),
        Err(AppError::BadModelOutput(_))
    ));
}

#[test]
fn transcript_numbers_main_path_messages() {
    let (_d, conn) = setup();
    let id = add_chat(&conn, "c1", 3);
    let input = tag::build_input(&conn, id).expect("input");
    assert_eq!(input.message_ids.len(), 3);
    assert!(input.transcript.contains("[0] PERSON"));
    assert!(input.transcript.contains("[1] ASSISTANT"));
    let msg = tag::user_message(&input, &["Baking".into()]);
    assert!(msg.contains("numbered 0 to 2"));
    assert!(msg.contains("Baking"));
}
