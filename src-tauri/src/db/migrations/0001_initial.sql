-- Initial schema: raw export data and full-text search.
-- Derived tables (segments, themes, tags, memory items) arrive in later
-- migrations alongside the features that fill them.

CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;

-- One row per successful import. Imports run in a single transaction, so a
-- failed or cancelled import leaves no row and no partial data.
CREATE TABLE imports (
  id                    INTEGER PRIMARY KEY,
  started_at            INTEGER NOT NULL,  -- unix ms
  finished_at           INTEGER,
  sources               TEXT NOT NULL,     -- JSON array of file names (no directories)
  conversations_seen    INTEGER NOT NULL DEFAULT 0,
  conversations_added   INTEGER NOT NULL DEFAULT 0,
  conversations_updated INTEGER NOT NULL DEFAULT 0,
  messages_added        INTEGER NOT NULL DEFAULT 0,
  messages_updated      INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE conversations (
  id              INTEGER PRIMARY KEY,
  uuid            TEXT NOT NULL UNIQUE,
  name            TEXT NOT NULL,
  created_at      TEXT NOT NULL,           -- RFC 3339 UTC, as exported
  updated_at      TEXT NOT NULL,
  -- User-owned: the importer never writes these after insert.
  state           TEXT NOT NULL DEFAULT 'untriaged'
                    CHECK (state IN ('untriaged', 'organised', 'archived')),
  excluded        INTEGER NOT NULL DEFAULT 0 CHECK (excluded IN (0, 1)),
  -- Derived from messages on every import.
  message_count   INTEGER NOT NULL DEFAULT 0, -- messages on the main path
  -- Import bookkeeping.
  raw_hash        TEXT NOT NULL,           -- sha256 of the whole exported object
  raw_json        TEXT NOT NULL,           -- exported object without chat_messages
  first_import_id INTEGER NOT NULL REFERENCES imports(id),
  last_import_id  INTEGER NOT NULL REFERENCES imports(id)
) STRICT;

CREATE INDEX conversations_updated_at ON conversations(updated_at);
CREATE INDEX conversations_state ON conversations(state, updated_at);

-- Messages form a tree (edits and retries create branches). `idx` is the
-- position on the main path, NULL for messages on other branches.
CREATE TABLE messages (
  id              INTEGER PRIMARY KEY,
  conversation_id INTEGER NOT NULL REFERENCES conversations(id),
  uuid            TEXT NOT NULL,
  parent_uuid     TEXT,                    -- NULL for roots
  sender          TEXT NOT NULL,
  text            TEXT NOT NULL,           -- built from text content blocks
  attachment_text TEXT NOT NULL DEFAULT '',
  created_at      TEXT NOT NULL,
  updated_at      TEXT,
  array_idx       INTEGER NOT NULL,        -- position in the export array
  idx             INTEGER,
  raw_json        TEXT NOT NULL,           -- the exported message, untouched
  UNIQUE (conversation_id, uuid)
) STRICT;

CREATE INDEX messages_main_path ON messages(conversation_id, idx);
CREATE INDEX messages_parent ON messages(conversation_id, parent_uuid);

CREATE VIRTUAL TABLE messages_fts USING fts5(
  text,
  attachment_text,
  content = 'messages',
  content_rowid = 'id',
  tokenize = 'unicode61 remove_diacritics 2',
  prefix = '2 3'
);

CREATE TRIGGER messages_fts_insert AFTER INSERT ON messages BEGIN
  INSERT INTO messages_fts(rowid, text, attachment_text)
  VALUES (new.id, new.text, new.attachment_text);
END;

CREATE TRIGGER messages_fts_delete AFTER DELETE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, text, attachment_text)
  VALUES ('delete', old.id, old.text, old.attachment_text);
END;

-- Only text changes touch the index, so main-path recalculation is cheap.
CREATE TRIGGER messages_fts_update AFTER UPDATE OF text, attachment_text ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, text, attachment_text)
  VALUES ('delete', old.id, old.text, old.attachment_text);
  INSERT INTO messages_fts(rowid, text, attachment_text)
  VALUES (new.id, new.text, new.attachment_text);
END;

-- Other export files (projects, design chats) kept verbatim. Each distinct
-- version is kept, so nothing from an earlier export is ever overwritten.
CREATE TABLE export_files (
  id              INTEGER PRIMARY KEY,
  path            TEXT NOT NULL,           -- path inside the zip
  kind            TEXT NOT NULL,           -- project, design_chat
  content_hash    TEXT NOT NULL,
  content         TEXT NOT NULL,
  first_import_id INTEGER NOT NULL REFERENCES imports(id),
  UNIQUE (path, content_hash)
) STRICT;
