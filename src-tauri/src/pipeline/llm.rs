//! The model behind one trait, so the pipeline can be tested without the
//! network and a local backend can be added later for `local_only` mode.
//!
//! The Anthropic backend uses raw HTTPS: there is no official Rust SDK.

use std::time::Duration;

use serde_json::{Value, json};

use crate::error::{AppError, AppResult};

pub const MODEL: &str = "claude-opus-5-5";
/// Segmenting and tagging is judgement work but not deep reasoning; medium
/// is the model's default, set explicitly so a default change cannot
/// silently change cost.
const EFFORT: &str = "medium";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
/// Lets the API retry a declined request on its recommended fallback model
/// instead of returning nothing.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const MAX_TOKENS: u32 = 16_000;
const MAX_ATTEMPTS: u32 = 4;

/// US dollars per million tokens for MODEL.
pub mod price {
    pub const INPUT: f64 = 4.0;
    pub const OUTPUT: f64 = 20.0;
    pub const CACHE_READ: f64 = 0.20;
    pub const CACHE_WRITE: f64 = 5.0;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
}

impl Usage {
    pub fn add(&mut self, other: Usage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
    }

    pub fn cost_usd(&self) -> f64 {
        (self.input_tokens as f64 * price::INPUT
            + self.output_tokens as f64 * price::OUTPUT
            + self.cache_read_tokens as f64 * price::CACHE_READ
            + self.cache_write_tokens as f64 * price::CACHE_WRITE)
            / 1_000_000.0
    }
}

pub struct LlmRequest<'a> {
    /// Stable instructions, cached between calls.
    pub system: &'a str,
    pub user: &'a str,
    pub schema: &'a Value,
}

#[derive(Debug)]
pub struct LlmResponse {
    pub json: Value,
    pub usage: Usage,
}

pub trait Llm: Sync {
    fn model(&self) -> &str;
    fn complete(&self, req: &LlmRequest) -> AppResult<LlmResponse>;
}

pub struct Anthropic {
    http: reqwest::blocking::Client,
    api_key: String,
}

impl Anthropic {
    pub fn new(api_key: String) -> AppResult<Self> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|e| AppError::Network(e.to_string()))?;
        Ok(Self { http, api_key })
    }

    fn send_once(&self, body: &Value) -> Result<Value, Attempt> {
        let response = self
            .http
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("anthropic-beta", FALLBACK_BETA)
            .json(body)
            .send()
            .map_err(|e| Attempt::Retry(AppError::Network(e.to_string()), None))?;

        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs);
        let value: Value = response
            .json()
            .map_err(|e| Attempt::Retry(AppError::Network(e.to_string()), retry_after))?;

        if status == 200 {
            return Ok(value);
        }
        // API error bodies describe the request problem, never the content.
        let message = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("no details")
            .to_owned();
        let err = AppError::Api { status, message };
        match status {
            408 | 429 | 500 | 502 | 503 | 504 | 529 => Err(Attempt::Retry(err, retry_after)),
            _ => Err(Attempt::Fail(err)),
        }
    }
}

enum Attempt {
    Retry(AppError, Option<Duration>),
    Fail(AppError),
}

impl Llm for Anthropic {
    fn model(&self) -> &str {
        MODEL
    }

    fn complete(&self, req: &LlmRequest) -> AppResult<LlmResponse> {
        let body = json!({
            "model": MODEL,
            "max_tokens": MAX_TOKENS,
            "system": [{
                "type": "text",
                "text": req.system,
                "cache_control": { "type": "ephemeral" }
            }],
            "messages": [{ "role": "user", "content": req.user }],
            "output_config": {
                "effort": EFFORT,
                "format": { "type": "json_schema", "schema": req.schema }
            },
            "fallbacks": "default"
        });

        let mut attempt = 0;
        let value = loop {
            attempt += 1;
            match self.send_once(&body) {
                Ok(v) => break v,
                Err(Attempt::Fail(e)) => return Err(e),
                Err(Attempt::Retry(e, after)) => {
                    if attempt >= MAX_ATTEMPTS {
                        return Err(e);
                    }
                    let backoff = Duration::from_secs(2u64.pow(attempt));
                    std::thread::sleep(after.unwrap_or(backoff).min(Duration::from_secs(60)));
                }
            }
        };
        parse_response(&value)
    }
}

/// Pulls the JSON answer and billed usage out of a Messages API response.
pub fn parse_response(value: &Value) -> AppResult<LlmResponse> {
    let usage = parse_usage(value.get("usage").unwrap_or(&Value::Null));

    match value.get("stop_reason").and_then(Value::as_str) {
        Some("refusal") => {
            return Err(AppError::BadModelOutput(
                "the model declined this chat".into(),
            ));
        }
        Some("max_tokens") => {
            return Err(AppError::BadModelOutput(
                "the answer was cut off at the token limit".into(),
            ));
        }
        _ => {}
    }

    let text: String = value
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect();
    let json = serde_json::from_str(&text)
        .map_err(|e| AppError::BadModelOutput(format!("not valid JSON ({e})")))?;
    Ok(LlmResponse { json, usage })
}

/// With fallbacks, top-level usage covers only the attempt that answered;
/// `usage.iterations` lists every billed attempt, so sum those when present.
fn parse_usage(usage: &Value) -> Usage {
    let one = |u: &Value| Usage {
        input_tokens: u.get("input_tokens").and_then(Value::as_i64).unwrap_or(0),
        output_tokens: u.get("output_tokens").and_then(Value::as_i64).unwrap_or(0),
        cache_read_tokens: u
            .get("cache_read_input_tokens")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        cache_write_tokens: u
            .get("cache_creation_input_tokens")
            .and_then(Value::as_i64)
            .unwrap_or(0),
    };
    match usage.get("iterations").and_then(Value::as_array) {
        Some(its) if !its.is_empty() => {
            let mut total = Usage::default();
            for it in its {
                total.add(one(it));
            }
            total
        }
        _ => one(usage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_json_and_usage() {
        let v = json!({
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "" },
                { "type": "text", "text": "{\"segments\": []}" }
            ],
            "usage": { "input_tokens": 100, "output_tokens": 20,
                       "cache_read_input_tokens": 5, "cache_creation_input_tokens": 0 }
        });
        let r = parse_response(&v).expect("parse");
        assert_eq!(r.json, json!({ "segments": [] }));
        assert_eq!(r.usage.input_tokens, 100);
        assert_eq!(r.usage.cache_read_tokens, 5);
    }

    #[test]
    fn sums_fallback_iterations() {
        let v = json!({
            "stop_reason": "end_turn",
            "content": [{ "type": "text", "text": "{}" }],
            "usage": { "input_tokens": 10, "output_tokens": 1, "iterations": [
                { "type": "message", "input_tokens": 50, "output_tokens": 5 },
                { "type": "fallback_message", "input_tokens": 50, "output_tokens": 30 }
            ]}
        });
        let r = parse_response(&v).expect("parse");
        assert_eq!(r.usage.input_tokens, 100);
        assert_eq!(r.usage.output_tokens, 35);
    }

    #[test]
    fn refusal_and_truncation_are_errors() {
        for reason in ["refusal", "max_tokens"] {
            let v = json!({ "stop_reason": reason, "content": [], "usage": {} });
            assert!(matches!(
                parse_response(&v),
                Err(AppError::BadModelOutput(_))
            ));
        }
    }

    #[test]
    fn cost_uses_model_prices() {
        let u = Usage {
            input_tokens: 1_000_000,
            output_tokens: 100_000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        assert!((u.cost_usd() - 6.0).abs() < 1e-9);
    }
}
