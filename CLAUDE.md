# CLAUDE.md

NOTE:

- exported files are in .temp/export
- never commit these files
- password protect this app (when accessing you should have to log in) - doesn't need to be OAuth or anything - just make a local account or password protect the encrypted data

## What this is

A desktop app (Tauri v2, macOS and Windows) that turns my chat history into something I can organise and use. It is a memory management tool, not a graph visualiser. The graph is one view among several, and not the main one.

The problem it solves: my chats are scattered, topics repeat without me noticing, decisions get lost, and ideas that belong together sit in unrelated chats. I want to see that, fix it, and be able to find things again.

Single user, local only, no accounts. Network use is limited to the update check and, in `api` mode only, calls to the Anthropic API (see "Privacy and modes").

I give it a Claude data export file and re-import a fresh one when I want updates. No live sync.

## Core principles

1. **My edits are authoritative.** If I rename a theme, merge two, move a segment or pin something, that survives every re-import and every re-run of tagging. Derived data is rebuildable. My corrections are not.
2. **Provenance everywhere.** Every theme, tag and memory item links back to the exact messages it came from, one click away.
3. **Useful before clever.** Browse and full-text search work before any LLM is involved.
4. **Honest metrics.** Report scatter plainly. No flattering framing.
5. **Not graph-first.** If a feature only makes sense as a graph, question it.

## Stack

- **Shell:** Tauri v2 (Rust backend)
- **Frontend:** React, TypeScript (strict), Tailwind CSS, shadcn/ui
- **State:** Zustand for UI state, TanStack Query for anything that comes from Rust commands
- **Database:** SQLite via `rusqlite` (with the `bundled` feature), FTS5 for search
- **Import:** `zip` crate for the export archive
- **Secrets:** `keyring` crate (macOS Keychain, Windows Credential Manager)
- **Graph view:** Cytoscape.js or sigma.js, whichever copes better with a few thousand nodes
- **Embeddings:** undecided. Evaluate `fastembed-rs` (bundled ONNX model) against a local Ollama, and propose one with the trade-offs (install size, speed, Windows support) before building.
- **Package manager:** pnpm
- **UI extras:** `react-resizable-panels` for the panes, `cmdk` for the command palette
- **Tooling:** `prettier`, `eslint` (flat config), `cargo fmt`, `cargo clippy`
- **Toolchain:** Node 20+, pnpm, a current stable Rust. Pin versions through the lockfiles. A newer Tauri release can require a newer Rust than is installed, so check before bumping, and record the minimum Rust version in the README.

Do not add Electron, a Node backend, or an ORM. Keep the Rust side thin and boring.

## Requirements

### Must have (v1)

1. **Import.** File picker for the export (zip or bare `conversations.json`). Idempotent, keyed on conversation uuid, handles conversations that gained messages since the last export. Unknown fields are kept raw, never dropped.
2. **Inbox and reader.** All chats by date with a state (untriaged, organised, archived). Fast with thousands of chats, so virtualise the lists.
3. **Search.** Full text over messages with FTS5, fast enough for every keystroke.
4. **Themes.** The main organising view: a browsable, editable structure of themes and sub-themes. Rename, merge, split, move segments between themes.
5. **Auto-update.** See below.

### Nice to have (in this order)

1. **Tagging pipeline:** segment chats, tag segments, propose themes (see "Pipeline").
2. **Review queue:** suggested merges, renames and low-confidence tags for me to accept or reject.
3. **Memory items:** decisions, preferences, open questions, projects and facts I want to keep, each linked to its source messages and confirmable, editable or dismissable. Open loops (unresolved questions, abandoned plans) get their own list.
4. **Thread view:** one theme laid out over time, showing how an idea developed and where it stalled.
5. **Semantic search** and clustering from embeddings.
6. **Graph view:** themes and segments as nodes, with edges from shared tags, similarity and callbacks. Good for spotting orphans and bridges.
7. **Metrics view** (see "Scatter metrics").

### Out of scope unless asked

Being a chat client, live sync from claude.ai, cloud features, multi-user, editing or deleting the original chats. Do not build these speculatively.

## Architecture

Repo root:

```
.github/workflows/   # release workflow
public/
scripts/             # version bump script and similar
src/                 # frontend
src-tauri/           # Rust backend
.gitignore
.prettierrc
.prettierignore
CLAUDE.md
README.md
LICENSE
app-icon.svg
eslint.config.js
index.html
package.json
pnpm-lock.yaml
tsconfig.json        # plus tsconfig.app.json and tsconfig.node.json
vite.config.ts
```

App structure:

```
src-tauri/
  src/
    main.rs
    commands/        # thin Tauri command handlers, one file per area
      import.rs
      chats.rs
      themes.rs
      memory.rs
      search.rs
      jobs.rs
      settings.rs
      updater.rs
    db/
      mod.rs         # connection, migrations, backup
      schema.sql
      queries.rs
    import/
      export.rs      # reads the export zip and conversations.json
    pipeline/
      segment.rs
      tag.rs
      extract.rs     # memory items
      embed.rs
      cluster.rs
      llm.rs         # api and local_only backends behind one trait
    secrets.rs       # keyring wrapper
src/
  components/
    inbox/
    themes/
    memory/
    thread/
    graph/
    metrics/
    review/
    layout/
    settings/        # includes About and updates
    ui/              # shadcn components
  features/
  lib/
  store/
  types/
```

Rust owns the filesystem, database, pipeline and secrets. The frontend never touches files directly. All data crosses the boundary through typed commands and events.

## Input

A Claude data export. Likely a zip containing `conversations.json`, possibly more files. Expected shape of the conversations, **unverified**: an array with uuid, name, created_at, updated_at and chat_messages (sender, text, created_at).

First job: inspect my real export and write the actual schema to `docs/export-format.md`. Do not build on the expected shape until then.

## Pipeline

1. Ingest into SQLite with an FTS index.
2. **Segment** long chats where the topic shifts. One chat often holds several unrelated topics, and this is central to the scatter question.
3. **Tag** segments: 1 to 3 themes, entities, intent type (debugging, philosophy, planning and so on). JSON output, cached by content hash. Reuse existing tags before creating new ones, and respect my edits.
4. **Extract memory items** with source message references.
5. **Embed**, cluster, and have an LLM propose theme names. Proposals go to the review queue and never overwrite anything I have edited.
6. Build graph edges.
7. Compute metrics.

Long jobs run in the background with progress events, and are cancellable and resumable.

## Data model

Sketch, to be refined after the export is inspected:

```
conversations(id, uuid, name, created_at, updated_at, state, raw_json)
messages(id, conversation_id, sender, text, created_at, idx, raw_json)
segments(id, conversation_id, start_idx, end_idx, source)   -- source: auto or user
themes(id, parent_id, name, origin, pinned)                  -- origin: auto or user
segment_themes(segment_id, theme_id, origin, confidence)
tags(id, name, origin)
segment_tags(segment_id, tag_id, origin)
memory_items(id, kind, text, status, origin)                 -- status: suggested, confirmed, dismissed
memory_sources(memory_item_id, message_id)
embeddings(segment_id, model, vector)
llm_cache(content_hash, task, model, result_json)
settings(key, value)
messages_fts   -- FTS5 over message text
```

Rules:

- Every derived row carries `origin` (auto or user). A re-run may replace `auto` rows. It must never touch `user` rows or rows I have confirmed.
- Never delete the raw export data.
- Numbered SQL migrations and a migrations table.

## Scatter metrics

- Distinct themes per chat, and topic switches within a chat.
- Distinct themes per week.
- Returns: themes dropped and later revisited, with the gap length.
- Orphans: chats that connect to nothing.
- Concentration: share of segments in my top 5 themes.
- Open loops: count and age.

## Privacy and modes

Chat content is sensitive. Two modes, set in settings and visible in the UI at all times:

- `local_only`: local embeddings and a local model. Nothing leaves the machine.
- `api`: tagging and extraction send chat text to the Anthropic API. Show a clear warning the first time, and let me exclude chats from processing.

The API key lives in the OS keychain, never in a config file. Never log message content. Never commit export data. The update check sends nothing about me or my data.

## UI guidelines

- Dark theme first, with light and system options. shadcn/ui and Tailwind.
- Layout built around the Themes view, with the inbox and reader alongside it. Resizable panes that remember their size.
- Command palette (`cmdk`) on Ctrl/Cmd+K.
- Keyboard first.
- Empty, loading and error states everywhere. No blank panes.
- Job progress in a status bar, never a blocking modal.
- UI text in British English.

## Auto-update

GitHub-hosted, signed and opt-out. Build it as follows.

Behaviour:

- On startup, check GitHub for a newer version in the background. Never delay startup. Fail silently when offline.
- If one exists, offer to install it, showing the version and release notes. Never install without asking.
- **Settings > About** shows the current version, a switch to turn the startup check off, and a "Check for updates" button.
- Updates are signed. The app only accepts bundles signed by the key whose public half is in `tauri.conf.json`.
- Do not restart for an update while a tagging, extraction or embedding job is running. Offer to install when idle.

Implementation notes (verify against current Tauri v2 docs):

- `tauri-plugin-updater`, plus `tauri-plugin-process` for relaunch.
- `bundle.createUpdaterArtifacts` enabled in `tauri.conf.json`.
- Endpoint: `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`, with the public key in the config.
- Grant only the updater and process permissions in the capabilities file.
- Keep the `tauri` crate and the `@tauri-apps/*` npm packages on the same minor version.

Data safety on update. This database holds my organised memory and my corrections, which cannot be regenerated:

- Back up the SQLite file before running any migration, and keep the last few backups.
- Migrations only move forward and must not destroy data.
- If the database was written by a newer app version than the one running, refuse to open it and explain, instead of risking corruption.
- Offer a manual "Back up now" and "Export my edits" in settings.

## Releasing

- Pushing a version tag (`v0.2.0`) triggers GitHub Actions to build the Windows and macOS installers, sign them for the updater, and publish a release that includes `latest.json`.
- Repository secrets: `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- The version lives in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`. Add a `pnpm bump <version>` script that changes all three, and a CI step that fails if the tag and the versions disagree.
- Generate a separate updater key for this app (do not share one between my two apps) and keep a backup outside the repo. If the key is lost, installed copies cannot update and need a manual reinstall.
- The first version that contains the updater has to be installed by hand.
- Updater signing is separate from OS code signing. Windows SmartScreen and macOS Gatekeeper warnings are expected until I sort out signing. Document them in the README.

## Git

I explicitly authorise git use in this project. This overrides any global rule of mine against using git, and applies to this repository only.

You may: run `git init`, check status and diffs, create branches, stage and commit, merge, and create tags. You may push to this project's own `origin` remote.

Ask first for: force pushes or any history rewriting (amending or rebasing commits that are already pushed, `reset --hard` over work I have not seen), deleting remote branches or tags, and pushing a version tag, because a tag push publishes a release.

Practice:

- Small commits, one concern each, with messages that say why. If a commit adds a dependency, say why.
- Check `git status` before every commit.
- Never commit secrets, the updater private key, `.env` files, session data or export data. Keep these in `.gitignore`: `.env*`, `*.key`, `data/`, `node_modules`, `src-tauri/target`, local database files.
- Test fixtures must be anonymised. Never commit real transcripts or exports.

## Conventions

- TypeScript strict mode, no `any`. Small components, plain hooks.
- Rust: no `unwrap()` outside tests. Commands return `Result<T, AppError>` with a serialisable error type the UI can show.
- Heavy work runs off the main thread (`tokio` tasks or `spawn_blocking`). The UI never freezes.
- Emit progress with Tauri events, not polling.
- Paths use `PathBuf` and OS APIs, never string concatenation with slashes.
- Format with `prettier` and `cargo fmt`. Lint with `eslint` and `cargo clippy`. Fix warnings, do not silence them.
- Do not add a dependency without a reason, and say why in the commit message.
- Tests for the parts that go wrong quietly: the importer (idempotence, updated conversations, unknown fields), segmentation, the rule that user edits survive re-runs, migrations and backup. UI tests are optional.
- Comments explain why, not what. British English. No em dashes.

## Commands

```
pnpm install
pnpm tauri dev
pnpm tauri build
pnpm lint
pnpm typecheck
pnpm bump <version>
cd src-tauri && cargo test
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

## Build order

Get each step usable before starting the next.

1. Scaffold Tauri v2 with React, TS, Tailwind and shadcn. Confirm a hello-world builds and runs on both macOS and Windows.
2. Inspect my real export, write `docs/export-format.md`.
3. SQLite schema and migrations, importer with tests.
4. Inbox, reader and full-text search. No AI yet. Check it is already useful.
5. Auto-update, release workflow, version bump script. Test a real update between two tagged releases early, because it is much harder to retrofit once there is data to protect.
6. Tag a sample of 20 chats. Show me the results and stop for review.
7. Full tagging, with a cost estimate first.
8. Themes view with editing, and the rule that my edits survive re-runs.
9. Review queue.
10. Memory items and open loops.
11. Embeddings, semantic search, clustering.
12. Graph view.
13. Metrics view.

## Rules for Claude

- Plan first for anything over about 50 lines. Show me the plan.
- Give a cost estimate before any bulk API run and wait for my go-ahead.
- Quality is checked by hand. I will label about 30 segments, and tagging and clustering get measured against them. Do not call it working on vibes.
- When you add derived data, say how my edits to it are preserved.
- Flag anything you could not verify on the other OS.
- Say plainly when results look poor. Do not tune until they merely look good.

## Things to ask me rather than guess

- Roughly how many chats are in my export. It decides how much effort goes into import and index performance.
- Whether macOS builds should be universal or separate arm64 and x64 artifacts.
- Whether in-app updates should be expected to work on macOS before notarisation. Test it and tell me what you find.
- Which embedding option to use, after you have laid out the trade-offs.
- Whether any chats should be excluded from processing by default.
