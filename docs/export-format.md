# Claude data export format

Observed from a real export taken on 2026-11-07. Only structure is recorded here. No content from the export is reproduced.

## Delivery

The export arrives as **several zip files**, not one. Each name has a `-NNN` suffix, which suggests larger exports may be split further (`conversations-001.zip` and so on). The importer must therefore accept a folder or several zips at once, and also a bare `conversations.json`.

| Zip                      | Contains                                                         | Used by v1            |
| ------------------------ | ---------------------------------------------------------------- | --------------------- |
| `conversations-000.zip`  | `conversations.json` (about 150 MB uncompressed for 1,354 chats) | Yes                   |
| `projects-000.zip`       | `projects/<uuid>.json`, one per project                          | Stored raw            |
| `design_chats-000.zip`   | `design_chats/<uuid>.json`, a different chat format              | Stored raw, see below |
| `frames-000.zip`         | `artifacts/<id>/artifact.json`, versioned HTML, `page.md`, etc.  | No                    |
| `artifact_data-000.zip`  | `artifacts/<id>/data/documents.json`                             | No                    |
| `light_metadata-000.zip` | `users.json`, `login_history.json` (IP addresses, user agents)   | No, never read        |

Zip entries all have the timestamp 1980-01-01, so the zip's own dates say nothing about when the export was made. The newest `updated_at` inside the data is the only reliable signal.

`login_history.json` holds IP addresses and is not needed for anything. The importer should skip it entirely.

## conversations.json

A single JSON array. The file is too large to treat casually (150 MB), so parse it with a streaming reader (`serde_json` `StreamDeserializer` over the array, or deserialise per element) rather than building one big `Value`.

### Conversation

| Field           | Type   | Notes                                                          |
| --------------- | ------ | -------------------------------------------------------------- |
| `uuid`          | string | Unique across the file. Import key.                            |
| `name`          | string | Can be empty (17 of 1,354).                                    |
| `summary`       | string | Present on every chat, **empty on every chat** in this export. |
| `created_at`    | string | RFC 3339, UTC, microseconds: `2026-02-03T20:39:43.162731Z`.    |
| `updated_at`    | string | Same format.                                                   |
| `account.uuid`  | string | Owner account. Same for every chat.                            |
| `chat_messages` | array  | Can be empty (3 chats).                                        |

There is **no project link** on a conversation, even though projects are exported. Conversations cannot be grouped by project from this data.

### Message

| Field                 | Type   | Notes                                                                         |
| --------------------- | ------ | ----------------------------------------------------------------------------- |
| `uuid`                | string | Message id.                                                                   |
| `parent_message_uuid` | string | Parent in the message tree. Roots use `00000000-0000-4000-8000-000000000000`. |
| `sender`              | string | `human` or `assistant`. No others seen.                                       |
| `text`                | string | Flattened text, see below.                                                    |
| `content`             | array  | Typed content blocks, see below.                                              |
| `created_at`          | string | Same format as above.                                                         |
| `updated_at`          | string |                                                                               |
| `attachments`         | array  | Pasted or uploaded text files with their extracted text.                      |
| `files`               | array  | `{file_uuid, file_name}` only. The file bytes are **not** exported.           |

### Messages form a tree, not a list

Each message points at its parent. Editing a prompt or retrying a reply creates a sibling branch.

- 55 conversations have at least one branch point.
- 16 conversations have more than one root.
- In 70 conversations the array order does not follow the parent chain.
- 37 places have two consecutive messages from the same sender, which is what you get when a branch is read as a flat list.

The export does not say which branch was the "current" one. Plan: store every message with its parent, and derive a **main path** by walking up from the most recent leaf. The reader shows the main path and marks branch points so other branches are reachable. Segmentation and tagging run on the main path only, to start with.

`idx` in the data model sketch becomes the position on the main path. Raw array order is kept as `array_idx` for provenance.

### `text` versus `content`

`content` is the structured form. `text` is a flattened rendering of it.

- In 14,059 messages, `text` equals the concatenation of the `text` blocks in `content`.
- In 2,767 messages it differs. In every one of these, `text` is longer and contains all the text blocks plus placeholders for non-text blocks: the literal `This block is not supported on your current device yet.` and code fences around tool calls.
- 370 human and 66 assistant messages have an empty `text`.

For search and tagging, build message text from the `text` blocks in `content`, not from `text`. Keep `text` in `raw_json`.

### Content block types

Every block has `type`, plus `start_timestamp` and `stop_timestamp` (usually present) and `flags` (always null).

| `type`         | Count  | Useful fields                                                                               |
| -------------- | ------ | ------------------------------------------------------------------------------------------- |
| `text`         | 19,258 | `text`, `citations[]` (web search citations with URLs).                                     |
| `tool_use`     | 8,527  | `id`, `name`, `input` (free-form object), `integration_name`, `message`, `display_content`. |
| `tool_result`  | 8,517  | `tool_use_id`, `name`, `content[]`, `is_error`, `structured_content`, `display_content`.    |
| `thinking`     | 3,869  | `thinking`, `summaries[]`, `truncated`, `cut_off`.                                          |
| `token_budget` | 37     | Nothing useful.                                                                             |

`tool_result.content[]` items have their own `type`: `knowledge` (web pages, with `metadata.type = webpage_metadata`), `text`, `local_resource`, `image` and `image_gallery`.

`tool_use.input` and `display_content` have open-ended shapes that vary by tool (web fetch, file creation, docs, MCP servers and so on). They are kept raw and not modelled.

Indexing choice for v1:

- **Indexed for search:** `text` blocks, and attachment `extracted_content` with the file name.
- **Stored, not indexed by default:** `thinking` blocks. They are model reasoning, not something I said or was told, and would add noise to search and tagging. A search filter can include them later.
- **Stored raw only:** tool use and results. Web page dumps would swamp the index.

### Attachments

`{file_name, file_size, file_type, extracted_content}`. 388 in total, mostly pasted text (`file_type` `txt`). `extracted_content` is full text, so it is as sensitive as message text.

## Scale

| Measure                      | Value                                    |
| ---------------------------- | ---------------------------------------- |
| Conversations                | 1,354                                    |
| Messages                     | 16,826 (8,423 human, 8,403 assistant)    |
| Messages per chat            | median 4, p90 29, p99 108, max 248       |
| Characters per chat (`text`) | median 4.5k, p90 31k, p99 120k, max 284k |
| Total message characters     | about 17.8 million                       |
| Date range                   | 2026-02-03 to 2026-11-07                 |

This is small for SQLite and FTS5. Import performance mostly comes down to streaming the 150 MB JSON and doing it in one transaction. List virtualisation is still worth having at this size.

## Other files

### projects/&lt;uuid&gt;.json

`{uuid, name, description, prompt_template, is_private, is_starter_project, created_at, updated_at, creator{uuid, full_name}, docs[{uuid, filename, content, created_at}]}`. Kept raw in v1. They could feed theme seeding later, but nothing links them to conversations.

### design_chats/&lt;uuid&gt;.json

A separate chat format: `{uuid, title, created_at, updated_at, project{uuid, name}, messages[{uuid, role, created_at, content{id, role, content, contentBlocks[], attachments[], timestamp, ...}}]}`. `role` is `user` or `assistant`. Only 4 files. Kept raw in v1. They could be mapped into the conversation tables later, with `role` mapped to `sender`.

### artifacts/

`artifact.json` is `{id, kind (artifact or page), active_version, owner_account, updated_at, visibility, versions[{id, title, description?, created_at}]}`, with versioned HTML, `page.md` and `comments.json` alongside. There is no link to the conversation that created them. Out of scope for v1.

## Unknown fields

Any field not modelled above stays in `raw_json` on the conversation or message row. The importer never drops fields. Future exports may add fields or new content block types. Unknown block types are stored and ignored by the text builder, never treated as an error.
