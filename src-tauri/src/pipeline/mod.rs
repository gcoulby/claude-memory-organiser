//! Background tagging runs.
//!
//! Each conversation is stored as soon as its answer arrives, in its own
//! transaction, so a cancelled or failed run keeps what it paid for and a
//! re-run picks up where it left off: conversations whose input is unchanged
//! since they were tagged are skipped, and answers already in the cache are
//! reused without calling the API.

pub mod llm;
pub mod tag;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use llm::{Llm, LlmRequest, Usage};
use tag::TagInput;

const WORKERS: usize = 4;
/// Rough characters per token, used only until a run has measured the real ratio.
const DEFAULT_CHARS_PER_TOKEN: f64 = 3.5;
/// Prompt and schema overhead per call, in tokens.
const OVERHEAD_TOKENS: f64 = 1_800.0;
/// Output (thinking plus JSON) per conversation, until measured.
const DEFAULT_OUTPUT_TOKENS: f64 = 2_500.0;
pub const SAMPLE_SIZE: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Sample,
    All,
}

/// Conversations a run would cover. Excluded chats are never included.
pub fn select(conn: &Connection, scope: Scope) -> AppResult<Vec<i64>> {
    let ids: Vec<i64> = conn
        .prepare(
            "SELECT id FROM conversations
             WHERE excluded = 0 AND message_count > 0
             ORDER BY updated_at",
        )?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(match scope {
        Scope::All => ids,
        Scope::Sample => {
            // Spread across the whole history rather than the newest 20, and
            // prefer chats with some substance.
            let substantial: Vec<i64> = conn
                .prepare(
                    "SELECT id FROM conversations
                     WHERE excluded = 0 AND message_count >= 4
                     ORDER BY updated_at",
                )?
                .query_map([], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            let pool = if substantial.len() >= SAMPLE_SIZE {
                substantial
            } else {
                ids
            };
            let step = (pool.len() as f64 / SAMPLE_SIZE as f64).max(1.0);
            (0..SAMPLE_SIZE.min(pool.len()))
                .filter_map(|i| pool.get((i as f64 * step) as usize).copied())
                .collect()
        }
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    pub conversations: usize,
    pub already_tagged: usize,
    pub cached: usize,
    pub to_send: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    /// True once a real run has calibrated the token ratios.
    pub calibrated: bool,
    pub model: &'static str,
}

enum Plan {
    Skip,
    Cached(serde_json::Value),
    Send,
}

fn plan(conn: &Connection, input: &TagInput, model: &str) -> AppResult<Plan> {
    let tagged: Option<String> = conn
        .prepare_cached("SELECT content_hash FROM tagged_conversations WHERE conversation_id = ?1 AND model = ?2")?
        .query_row((input.conversation_id, model), |r| r.get(0))
        .optional()?;
    if tagged.as_deref() == Some(input.hash.as_str()) {
        return Ok(Plan::Skip);
    }
    let cached: Option<String> = conn
        .prepare_cached("SELECT result_json FROM llm_cache WHERE content_hash = ?1 AND task = ?2 AND model = ?3")?
        .query_row((&input.hash, tag::TASK, model), |r| r.get(0))
        .optional()?;
    Ok(match cached {
        Some(json) => Plan::Cached(serde_json::from_str(&json)?),
        None => Plan::Send,
    })
}

pub fn estimate(conn: &Connection, ids: &[i64]) -> AppResult<Estimate> {
    // Calibrate from earlier answers when there are any.
    let (chars, input, output, n): (i64, i64, i64, i64) = conn.query_row(
        "SELECT coalesce(sum(input_chars), 0), coalesce(sum(input_tokens), 0),
                coalesce(sum(output_tokens), 0), count(*)
         FROM llm_cache WHERE model = ?1",
        [llm::MODEL],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    let calibrated = n >= 5;
    let (chars_per_token, overhead, output_per_conv) = if calibrated {
        // Overhead is folded into the ratio once calibrated.
        (
            chars as f64 / input.max(1) as f64,
            0.0,
            output as f64 / n as f64,
        )
    } else {
        (
            DEFAULT_CHARS_PER_TOKEN,
            OVERHEAD_TOKENS,
            DEFAULT_OUTPUT_TOKENS,
        )
    };

    let mut est = Estimate {
        conversations: ids.len(),
        already_tagged: 0,
        cached: 0,
        to_send: 0,
        input_tokens: 0,
        output_tokens: 0,
        cost_usd: 0.0,
        calibrated,
        model: llm::MODEL,
    };
    for &id in ids {
        let input = tag::build_input(conn, id)?;
        match plan(conn, &input, llm::MODEL)? {
            Plan::Skip => est.already_tagged += 1,
            Plan::Cached(_) => est.cached += 1,
            Plan::Send => {
                est.to_send += 1;
                est.input_tokens +=
                    (input.transcript.len() as f64 / chars_per_token + overhead) as i64;
                est.output_tokens += output_per_conv as i64;
            }
        }
    }
    est.cost_usd = Usage {
        input_tokens: est.input_tokens,
        output_tokens: est.output_tokens,
        ..Usage::default()
    }
    .cost_usd();
    Ok(est)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagProgress {
    pub done: usize,
    pub total: usize,
    pub failed: usize,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSummary {
    pub run_id: i64,
    pub tagged: usize,
    pub skipped: usize,
    pub from_cache: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub repairs: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    /// One line per failure, never containing chat content.
    pub errors: Vec<String>,
}

/// A bad key or no credit: every other call would fail the same way.
fn is_fatal(err: &AppError) -> bool {
    match err {
        AppError::Api { status, message } => {
            matches!(status, 401 | 403) || (*status == 400 && message.contains("credit"))
        }
        AppError::NoApiKey => true,
        _ => false,
    }
}

struct Job {
    input: TagInput,
    themes: Vec<String>,
}

/// Runs tagging over `ids`. Blocks until done; call from a background thread.
pub fn run(
    conn: &mut Connection,
    llm: &dyn Llm,
    ids: &[i64],
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(&TagProgress),
) -> AppResult<TagSummary> {
    let model = llm.model().to_owned();
    conn.execute(
        "INSERT INTO llm_runs (task, model, started_at, status) VALUES (?1, ?2, ?3, 'running')",
        (tag::TASK, &model, now_ms()),
    )?;
    let run_id = conn.last_insert_rowid();
    let mut summary = TagSummary {
        run_id,
        tagged: 0,
        skipped: 0,
        from_cache: 0,
        failed: 0,
        cancelled: false,
        repairs: 0,
        input_tokens: 0,
        output_tokens: 0,
        cost_usd: 0.0,
        errors: Vec::new(),
    };

    // Sort out what needs the API before starting any calls.
    let mut to_send = Vec::new();
    for &id in ids {
        let input = tag::build_input(conn, id)?;
        match plan(conn, &input, &model)? {
            Plan::Skip => summary.skipped += 1,
            Plan::Cached(json) => {
                let n = input.message_ids.len();
                match tag::validate(&json, n) {
                    Ok(result) => {
                        let tx = conn.transaction()?;
                        tag::store(&tx, &input, &result, &model, Some(run_id))?;
                        tx.commit()?;
                        summary.from_cache += 1;
                    }
                    Err(e) => summary.errors.push(format!("chat {id}: {e}")),
                }
            }
            Plan::Send => to_send.push(input),
        }
    }

    let total = to_send.len();
    let vocabulary = Mutex::new(tag::theme_vocabulary(conn)?);
    let queue = Mutex::new(to_send.into_iter());
    let system = tag::SYSTEM_PROMPT;
    let schema = tag::schema();
    let (tx_results, rx_results) = mpsc::channel::<(TagInput, AppResult<llm::LlmResponse>)>();
    let fatal = AtomicBool::new(false);
    let mut usage = Usage::default();
    let mut outcome: AppResult<()> = Ok(());

    std::thread::scope(|scope| {
        for _ in 0..WORKERS.min(total) {
            let tx_results = tx_results.clone();
            let (queue, vocabulary, schema, fatal) = (&queue, &vocabulary, &schema, &fatal);
            scope.spawn(move || {
                loop {
                    if cancel.load(Ordering::Relaxed) || fatal.load(Ordering::Relaxed) {
                        break;
                    }
                    let next = queue.lock().unwrap_or_else(|e| e.into_inner()).next();
                    let Some(input) = next else { break };
                    let job = Job {
                        themes: vocabulary.lock().unwrap_or_else(|e| e.into_inner()).clone(),
                        input,
                    };
                    let user = tag::user_message(&job.input, &job.themes);
                    let result = llm.complete(&LlmRequest {
                        system,
                        user: &user,
                        schema,
                    });
                    // Stop the other workers at once, not when the main
                    // thread gets round to reading this result.
                    if result.as_ref().is_err_and(is_fatal) {
                        fatal.store(true, Ordering::Relaxed);
                    }
                    if tx_results.send((job.input, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx_results);

        // Results are written here, on the one thread that owns the connection.
        for (i, (input, result)) in rx_results.into_iter().enumerate() {
            let done = i + 1;
            let id = input.conversation_id;
            match result {
                Ok(response) => {
                    usage.add(response.usage);
                    let stored = (|| -> AppResult<usize> {
                        let checked = tag::validate(&response.json, input.message_ids.len())?;
                        let tx = conn.transaction()?;
                        tx.execute(
                            "INSERT OR REPLACE INTO llm_cache
                               (content_hash, task, model, result_json, input_chars, input_tokens, output_tokens, created_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                            (
                                &input.hash,
                                tag::TASK,
                                &model,
                                response.json.to_string(),
                                i64::try_from(input.transcript.len()).unwrap_or(i64::MAX),
                                response.usage.input_tokens
                                    + response.usage.cache_read_tokens
                                    + response.usage.cache_write_tokens,
                                response.usage.output_tokens,
                                now_ms(),
                            ),
                        )?;
                        tag::store(&tx, &input, &checked, &model, Some(run_id))?;
                        tx.commit()?;
                        let mut vocab = vocabulary.lock().unwrap_or_else(|e| e.into_inner());
                        for seg in &checked.segments {
                            for (name, _) in &seg.themes {
                                if !vocab.iter().any(|v| v.eq_ignore_ascii_case(name)) {
                                    vocab.push(name.clone());
                                }
                            }
                        }
                        Ok(checked.repairs.len())
                    })();
                    match stored {
                        Ok(repairs) => {
                            summary.tagged += 1;
                            summary.repairs += repairs;
                        }
                        Err(e) => {
                            summary.failed += 1;
                            summary.errors.push(format!("chat {id}: {e}"));
                        }
                    }
                }
                Err(e) => {
                    summary.failed += 1;
                    if is_fatal(&e) && outcome.is_ok() {
                        if let AppError::Api { status, message } = &e {
                            outcome = Err(AppError::Api {
                                status: *status,
                                message: message.clone(),
                            });
                        } else {
                            outcome = Err(AppError::NoApiKey);
                        }
                    }
                    summary.errors.push(format!("chat {id}: {e}"));
                }
            }
            let _ = conn.execute(
                "UPDATE llm_runs SET conversations_done = ?1, conversations_failed = ?2,
                   input_tokens = ?3, output_tokens = ?4, cache_read_tokens = ?5, cache_write_tokens = ?6
                 WHERE id = ?7",
                (
                    i64::try_from(summary.tagged).unwrap_or(0),
                    i64::try_from(summary.failed).unwrap_or(0),
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.cache_read_tokens,
                    usage.cache_write_tokens,
                    run_id,
                ),
            );
            on_progress(&TagProgress {
                done,
                total,
                failed: summary.failed,
                cost_usd: usage.cost_usd(),
            });
        }
    });

    summary.cancelled = cancel.load(Ordering::Relaxed);
    summary.input_tokens = usage.input_tokens + usage.cache_read_tokens + usage.cache_write_tokens;
    summary.output_tokens = usage.output_tokens;
    summary.cost_usd = usage.cost_usd();
    let status = if outcome.is_err() {
        "failed"
    } else if summary.cancelled {
        "cancelled"
    } else {
        "completed"
    };
    conn.execute(
        "UPDATE llm_runs SET status = ?1, finished_at = ?2 WHERE id = ?3",
        (status, now_ms(), run_id),
    )?;
    outcome.map(|()| summary)
}

#[cfg(test)]
mod tests;
