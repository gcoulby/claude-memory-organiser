//! Full-text search over message text and attachments.
//!
//! FTS5 finds and ranks matches. Snippets are built here rather than with
//! FTS5's `snippet()`, which took seconds on common words because it scores
//! every window of every long message.

use rusqlite::Connection;
use serde::Serialize;

use crate::error::AppResult;

/// Wrap matched words in snippets. Control characters never occur in the
/// words we match, and the UI splits on them instead of rendering HTML, so
/// message content cannot inject markup.
const MARK_START: char = '\u{2}';
const MARK_END: char = '\u{3}';
const WORDS_BEFORE: usize = 8;
const WORDS_AFTER: usize = 28;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub message_id: i64,
    pub message_uuid: String,
    pub conversation_id: i64,
    pub conversation_name: String,
    pub sender: String,
    pub created_at: String,
    pub on_main_path: bool,
    pub in_attachment: bool,
    pub snippet: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub total: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Term {
    pub word: String,
    pub prefix: bool,
}

#[derive(Debug)]
pub struct Query {
    pub fts: String,
    pub terms: Vec<Term>,
}

fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

/// Turns what the user typed into a safe FTS5 query.
///
/// Words become quoted terms joined by AND, so FTS5 syntax in the input
/// (quotes, NEAR, column filters, stray operators) can never cause a syntax
/// error. Text inside double quotes is kept together as a phrase.
pub fn build_query(input: &str) -> Option<Query> {
    let mut fts = Vec::new();
    let mut terms = Vec::new();
    for (i, part) in input.split('"').enumerate() {
        let ws: Vec<&str> = words(part).collect();
        if ws.is_empty() {
            continue;
        }
        // Odd-numbered parts sit between quotes.
        if i % 2 == 1 {
            fts.push(format!("\"{}\"", ws.join(" ")));
            terms.extend(ws.iter().map(|w| Term {
                word: w.to_lowercase(),
                prefix: false,
            }));
        } else {
            for w in ws {
                // A one-letter prefix matches most of the vocabulary: slow
                // and useless. Single letters only match themselves.
                let prefix = w.chars().count() > 1;
                fts.push(if prefix {
                    format!("\"{w}\"*")
                } else {
                    format!("\"{w}\"")
                });
                terms.push(Term {
                    word: w.to_lowercase(),
                    prefix,
                });
            }
        }
    }
    (!fts.is_empty()).then(|| Query {
        fts: fts.join(" "),
        terms,
    })
}

fn matches(word: &str, terms: &[Term]) -> bool {
    let lower = word.to_lowercase();
    terms.iter().any(|t| {
        if t.prefix {
            lower.starts_with(&t.word)
        } else {
            lower == t.word
        }
    })
}

/// A short window around the first match, with matched words marked.
/// Returns None when no word matches (for example, a diacritic-folded match).
pub fn make_snippet(text: &str, terms: &[Term]) -> Option<String> {
    // Byte ranges of each word in the text.
    let mut spans = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_alphanumeric(), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                spans.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        spans.push((s, text.len()));
    }

    let first = spans
        .iter()
        .position(|&(s, e)| matches(&text[s..e], terms))?;
    let from = first.saturating_sub(WORDS_BEFORE);
    let to = (first + WORDS_AFTER).min(spans.len() - 1);

    let mut out = String::new();
    if from > 0 {
        out.push('…');
    }
    let mut cursor = spans[from].0;
    for &(s, e) in &spans[from..=to] {
        out.push_str(&text[cursor..s]);
        let word = &text[s..e];
        if matches(word, terms) {
            out.push(MARK_START);
            out.push_str(word);
            out.push(MARK_END);
        } else {
            out.push_str(word);
        }
        cursor = e;
    }
    if to + 1 < spans.len() {
        out.push('…');
    }
    // Snippets are one line in the results list.
    Some(out.split_whitespace().collect::<Vec<_>>().join(" "))
}

pub fn search(conn: &Connection, input: &str, limit: i64) -> AppResult<SearchResults> {
    let Some(query) = build_query(input) else {
        return Ok(SearchResults {
            hits: Vec::new(),
            total: 0,
        });
    };

    let total = conn
        .prepare_cached("SELECT count(*) FROM messages_fts WHERE messages_fts MATCH ?1")?
        .query_row([&query.fts], |r| r.get(0))?;

    let mut stmt = conn.prepare_cached(
        "WITH top AS (
           SELECT rowid AS id, rank FROM messages_fts
           WHERE messages_fts MATCH ?1
           ORDER BY rank
           LIMIT ?2
         )
         SELECT m.id, m.uuid, m.conversation_id, c.name, m.sender, m.created_at,
                m.idx IS NOT NULL, m.text, m.attachment_text
         FROM top
         JOIN messages m ON m.id = top.id
         JOIN conversations c ON c.id = m.conversation_id
         ORDER BY top.rank",
    )?;
    let rows = stmt.query_map((&query.fts, limit), |r| {
        Ok((
            SearchHit {
                message_id: r.get(0)?,
                message_uuid: r.get(1)?,
                conversation_id: r.get(2)?,
                conversation_name: r.get(3)?,
                sender: r.get(4)?,
                created_at: r.get(5)?,
                on_main_path: r.get(6)?,
                in_attachment: false,
                snippet: String::new(),
            },
            r.get::<_, String>(7)?,
            r.get::<_, String>(8)?,
        ))
    })?;

    let mut hits = Vec::new();
    for row in rows {
        let (mut hit, text, attachment) = row?;
        if let Some(s) = make_snippet(&text, &query.terms) {
            hit.snippet = s;
        } else if let Some(s) = make_snippet(&attachment, &query.terms) {
            hit.snippet = s;
            hit.in_attachment = true;
        } else {
            // FTS matched after folding diacritics; show the opening instead.
            hit.snippet = text.chars().take(160).collect();
        }
        hits.push(hit);
    }
    Ok(SearchResults { hits, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fts(input: &str) -> Option<String> {
        build_query(input).map(|q| q.fts)
    }

    #[test]
    fn builds_prefix_terms_and_phrases() {
        assert_eq!(fts("sour dough").as_deref(), Some(r#""sour"* "dough"*"#));
        assert_eq!(
            fts(r#"rye "feeding schedule""#).as_deref(),
            Some(r#""rye"* "feeding schedule""#)
        );
        assert_eq!(fts("a cat").as_deref(), Some(r#""a" "cat"*"#));
        assert_eq!(fts("   ").as_deref(), None);
        assert_eq!(fts("--").as_deref(), None);
    }

    #[test]
    fn hostile_input_is_neutralised() {
        // Each of these is an FTS5 syntax error if passed through raw.
        for input in [
            r#"""#,
            "AND",
            "NEAR(",
            "text:foo",
            "a OR",
            "(",
            "*",
            r#""unclosed"#,
        ] {
            if let Some(q) = fts(input) {
                assert!(q.starts_with('"'), "{input:?} gave {q:?}");
            }
        }
    }

    #[test]
    fn hostile_input_runs_without_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = crate::db::open(
            &dir.path().join("m.db"),
            &crate::db::vault::DbKey::for_tests(7),
            &dir.path().join("b"),
        )
        .expect("open");
        for input in [
            r#"""#,
            "AND",
            "NEAR(",
            "text:foo",
            "a OR",
            "(",
            "*",
            "ünïcödé",
        ] {
            search(&conn, input, 10).expect(input);
        }
    }

    #[test]
    fn snippet_marks_matches_and_trims_long_text() {
        let terms = build_query("feed").expect("q").terms;
        let text = format!(
            "{} Feeding the starter daily. {}",
            "word ".repeat(40),
            "tail ".repeat(40)
        );
        let s = make_snippet(&text, &terms).expect("snippet");
        assert!(s.starts_with('…') && s.ends_with('…'));
        assert!(s.contains("\u{2}Feeding\u{3}"));
        assert!(!s.contains('\n'));
        assert!(make_snippet("nothing here", &terms).is_none());
    }
}
