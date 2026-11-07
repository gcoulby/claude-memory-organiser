-- Derived data from tagging. Every row carries `origin` (or `source`/`status`):
-- a re-run may replace rows the pipeline made ('auto', 'suggested') and must
-- never touch rows the user made or confirmed.

-- Topic-coherent stretches of a conversation's main path.
CREATE TABLE segments (
  id              INTEGER PRIMARY KEY,
  conversation_id INTEGER NOT NULL REFERENCES conversations(id),
  start_idx       INTEGER NOT NULL,          -- inclusive, main-path positions
  end_idx         INTEGER NOT NULL,          -- inclusive
  title           TEXT NOT NULL,
  summary         TEXT NOT NULL DEFAULT '',
  intent          TEXT NOT NULL DEFAULT 'other',
  source          TEXT NOT NULL CHECK (source IN ('auto', 'user')),
  run_id          INTEGER REFERENCES llm_runs(id),
  CHECK (end_idx >= start_idx)
) STRICT;

CREATE INDEX segments_conversation ON segments(conversation_id, start_idx);

-- Themes are the user's main organising structure. A merged theme points at
-- the theme it was merged into, so later runs that propose the old name land
-- in the right place.
CREATE TABLE themes (
  id          INTEGER PRIMARY KEY,
  parent_id   INTEGER REFERENCES themes(id),
  name        TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  origin      TEXT NOT NULL CHECK (origin IN ('auto', 'user')),
  pinned      INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
  merged_into INTEGER REFERENCES themes(id),
  created_at  INTEGER NOT NULL
) STRICT;

CREATE UNIQUE INDEX themes_name ON themes(name COLLATE NOCASE);

CREATE TABLE segment_themes (
  segment_id INTEGER NOT NULL REFERENCES segments(id) ON DELETE CASCADE,
  theme_id   INTEGER NOT NULL REFERENCES themes(id),
  origin     TEXT NOT NULL CHECK (origin IN ('auto', 'user')),
  confidence REAL,
  PRIMARY KEY (segment_id, theme_id)
) STRICT;

CREATE INDEX segment_themes_theme ON segment_themes(theme_id);

-- Entities: named things a segment is about (tools, people, products).
CREATE TABLE tags (
  id     INTEGER PRIMARY KEY,
  name   TEXT NOT NULL,
  origin TEXT NOT NULL CHECK (origin IN ('auto', 'user'))
) STRICT;

CREATE UNIQUE INDEX tags_name ON tags(name COLLATE NOCASE);

CREATE TABLE segment_tags (
  segment_id INTEGER NOT NULL REFERENCES segments(id) ON DELETE CASCADE,
  tag_id     INTEGER NOT NULL REFERENCES tags(id),
  origin     TEXT NOT NULL CHECK (origin IN ('auto', 'user')),
  PRIMARY KEY (segment_id, tag_id)
) STRICT;

-- Decisions, preferences, open questions, projects and facts worth keeping.
-- 'suggested' items are the pipeline's; 'confirmed' and 'dismissed' are the
-- user's verdicts and survive every re-run.
CREATE TABLE memory_items (
  id              INTEGER PRIMARY KEY,
  conversation_id INTEGER NOT NULL REFERENCES conversations(id),
  segment_id      INTEGER REFERENCES segments(id) ON DELETE SET NULL,
  kind            TEXT NOT NULL
                    CHECK (kind IN ('decision', 'preference', 'open_question', 'project', 'fact')),
  text            TEXT NOT NULL,
  status          TEXT NOT NULL DEFAULT 'suggested'
                    CHECK (status IN ('suggested', 'confirmed', 'dismissed')),
  origin          TEXT NOT NULL CHECK (origin IN ('auto', 'user')),
  created_at      INTEGER NOT NULL,
  run_id          INTEGER REFERENCES llm_runs(id)
) STRICT;

CREATE INDEX memory_items_kind ON memory_items(kind, status);
CREATE INDEX memory_items_conversation ON memory_items(conversation_id);

CREATE TABLE memory_sources (
  memory_item_id INTEGER NOT NULL REFERENCES memory_items(id) ON DELETE CASCADE,
  message_id     INTEGER NOT NULL REFERENCES messages(id),
  PRIMARY KEY (memory_item_id, message_id)
) STRICT;

-- One row per tagging run, for cost reporting.
CREATE TABLE llm_runs (
  id                  INTEGER PRIMARY KEY,
  task                TEXT NOT NULL,
  model               TEXT NOT NULL,
  started_at          INTEGER NOT NULL,
  finished_at         INTEGER,
  status              TEXT NOT NULL CHECK (status IN ('running', 'completed', 'cancelled', 'failed')),
  conversations_done  INTEGER NOT NULL DEFAULT 0,
  conversations_failed INTEGER NOT NULL DEFAULT 0,
  input_tokens        INTEGER NOT NULL DEFAULT 0,
  output_tokens       INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens   INTEGER NOT NULL DEFAULT 0,
  cache_write_tokens  INTEGER NOT NULL DEFAULT 0
) STRICT;

-- Model results keyed by what was sent, so re-runs and resumed runs never
-- pay twice for the same input.
CREATE TABLE llm_cache (
  content_hash  TEXT NOT NULL,
  task          TEXT NOT NULL,
  model         TEXT NOT NULL,
  result_json   TEXT NOT NULL,
  input_chars   INTEGER NOT NULL,          -- for calibrating cost estimates
  input_tokens  INTEGER NOT NULL,
  output_tokens INTEGER NOT NULL,
  created_at    INTEGER NOT NULL,
  PRIMARY KEY (content_hash, task, model)
) STRICT;

-- Which conversations have been tagged, from what input. A conversation
-- whose main path changes (new messages) gets a new hash and is re-tagged.
CREATE TABLE tagged_conversations (
  conversation_id INTEGER PRIMARY KEY REFERENCES conversations(id),
  content_hash    TEXT NOT NULL,
  model           TEXT NOT NULL,
  tagged_at       INTEGER NOT NULL
) STRICT;
