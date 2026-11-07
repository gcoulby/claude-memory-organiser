# Memory Organiser

A desktop app for organising a Claude chat export: themes, search, and the decisions and open questions buried in old chats. Local only. Built with Tauri v2, React and SQLite.

## Requirements

- Node 20 or later, and pnpm
- Rust **1.90 or later** (required by Tauri 2.12)
- macOS: Xcode command line tools
- Windows: the Microsoft C++ build tools and WebView2 (preinstalled on Windows 11)

Only Apple Silicon is supported on macOS.

## Development

```sh
pnpm install
pnpm tauri dev        # run the app with hot reload
pnpm tauri build      # build installers
pnpm lint             # eslint and prettier
pnpm typecheck
cd src-tauri && cargo test
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

## Your data

Everything is stored in one encrypted SQLite database (SQLCipher), unlocked with a password you set on first run. The key is derived with Argon2id. **There is no password recovery.** If the password is lost, the database cannot be opened and your edits are gone. You can still re-import the export, but organisation work would be lost.

Data lives in the app data folder:

- macOS: `~/Library/Application Support/com.gcoulby.memoryorganiser/`
- Windows: `%APPDATA%\com.gcoulby.memoryorganiser\`

It holds `memory.db`, `vault.json` (salt and KDF settings, no secrets) and `backups/`, which keeps the last five automatic backups taken before each schema migration.

To import, choose Import and select every zip from the Claude export (or just `conversations.json`). Re-importing a newer export is safe: unchanged chats are skipped, new messages are added, and your own organisation is never overwritten.

To check the importer against a real export without the UI (prints counts only):

```sh
cd src-tauri && MO_EXPORT_DIR=../.temp/export cargo test --release real_export -- --ignored --nocapture
```

The export format is documented in [docs/export-format.md](docs/export-format.md). Never commit export files. The `.temp/` folder is ignored for that reason.

## AI tagging

Tagging splits each chat into topic segments, files them under themes, and pulls out decisions, open questions, projects, preferences and facts, each linked to the messages it came from.

It is off by default ("Local only": nothing leaves the machine). In Settings you can switch to API mode, which sends the text of the chats you tag (including pasted attachments) to the Anthropic API using `claude-opus-5-5`. The API key is kept in the system keychain. Chats marked "Never send to AI" in the reader are always skipped.

Every run shows an estimate first and starts only when you confirm. Answers are cached by content, so re-running costs nothing for unchanged chats; only chats that gained messages are sent again. Your own decisions survive re-runs: confirmed and dismissed memory items stay as they are, a segmentation you have edited is kept, and merged themes stay merged.

On the author's export (1,354 chats) the first estimate was about $1.45 for the 20-chat sample and about $107 for everything; the estimate recalibrates from real token counts once a run has finished.

## Unsigned builds

The installers are not code signed with an Apple or Microsoft certificate yet, so the operating system warns on first launch:

- **macOS:** Gatekeeper says the app cannot be opened because the developer cannot be verified. Right-click the app, choose Open, then confirm.
- **Windows:** SmartScreen shows "Windows protected your PC". Choose More info, then Run anyway.

This is separate from updater signing, which is in place regardless.

## Releasing and updates

The app checks GitHub for a newer release at startup (switchable in Settings), offers to install it with the release notes, and never installs without asking. Updates are signed; the app only accepts bundles signed by the key whose public half is in `src-tauri/tauri.conf.json`. It will not offer to restart while an import is running.

To release:

```sh
pnpm bump 0.2.0          # sets the version in package.json, Cargo.toml, Cargo.lock and tauri.conf.json
git commit -am "Release 0.2.0"
git tag v0.2.0
git push origin main v0.2.0
```

Pushing the tag runs `.github/workflows/release.yml`: it checks the tag matches the version files, builds Apple Silicon macOS and Windows installers, signs the update bundles, and publishes the release with `latest.json` only after every platform has uploaded its files.

Repository secrets: `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. This app shares its updater key with Claude Task Manager. If that key is lost, installed copies of both apps can no longer update and need a manual reinstall, so keep a backup of it outside both repositories.

The first version that contains the updater has to be installed by hand. Local `pnpm tauri build` skips updater bundles; release builds add them through `src-tauri/tauri.release.conf.json`.
