//! Reading a Claude export from disk. See docs/export-format.md.
//!
//! An export arrives as several zips. We accept any mix of zip files, a
//! folder holding them, or a bare `conversations.json`.

use std::cell::Cell;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use serde_json::Value;

use crate::error::{AppError, AppResult};

/// Largest side file (project, design chat) we will load into memory.
const MAX_SIDE_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Expands folders into the export files they contain (not recursive).
pub fn collect_sources(paths: &[PathBuf]) -> AppResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut found: Vec<PathBuf> = fs::read_dir(path)?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| is_zip(p) || is_conversations_json(p))
                .collect();
            found.sort();
            out.extend(found);
        } else if is_zip(path) || is_conversations_json(path) {
            out.push(path.clone());
        } else {
            return Err(AppError::InvalidInput(format!(
                "{} is not a zip or conversations.json",
                display_name(path)
            )));
        }
    }
    if out.is_empty() {
        return Err(AppError::InvalidInput(
            "No export files found. Choose the export zips or conversations.json.".into(),
        ));
    }
    Ok(out)
}

fn is_zip(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

fn is_conversations_json(p: &Path) -> bool {
    p.file_name().is_some_and(|n| n == "conversations.json")
}

/// File name only, so paths (which can reveal folder names) stay out of
/// the database and error messages.
pub fn display_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// What a zip entry is, judged by its path.
#[derive(Debug, PartialEq, Eq)]
pub enum EntryKind {
    Conversations,
    Project,
    DesignChat,
    Ignored,
}

pub fn classify_entry(name: &str) -> EntryKind {
    let name = name.trim_start_matches("./");
    let in_dir = |dir: &str| {
        name.strip_prefix(dir)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|file| !file.contains('/') && file.ends_with(".json"))
    };
    if name == "conversations.json" || name.ends_with("/conversations.json") {
        EntryKind::Conversations
    } else if in_dir("projects") {
        EntryKind::Project
    } else if in_dir("design_chats") {
        EntryKind::DesignChat
    } else {
        // Includes login_history.json and users.json, which hold personal
        // data the app has no use for, and artifacts, out of scope for v1.
        EntryKind::Ignored
    }
}

/// Things the importer needs from each part of an export.
pub trait ExportSink {
    fn conversation(&mut self, value: Value) -> AppResult<()>;
    fn side_file(&mut self, path: &str, kind: &EntryKind, content: String) -> AppResult<()>;
    fn progress(&mut self, file: &str, bytes_done: u64, bytes_total: u64);
}

/// Reads every source in order, handing conversations and side files to the sink.
pub fn read_sources(sources: &[PathBuf], sink: &mut dyn ExportSink) -> AppResult<()> {
    for source in sources {
        let name = display_name(source);
        if is_zip(source) {
            read_zip(source, &name, sink)?;
        } else {
            let total = fs::metadata(source)?.len();
            let file = BufReader::new(File::open(source)?);
            stream_conversations(file, &name, total, sink)?;
        }
    }
    Ok(())
}

fn read_zip(path: &Path, name: &str, sink: &mut dyn ExportSink) -> AppResult<()> {
    let mut archive = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if !entry.is_file() {
            continue;
        }
        let entry_name = entry.name()?.into_owned();
        match classify_entry(&entry_name) {
            EntryKind::Conversations => {
                let total = entry.size();
                stream_conversations(BufReader::new(&mut entry), name, total, sink)?;
            }
            kind @ (EntryKind::Project | EntryKind::DesignChat) => {
                if entry.size() > MAX_SIDE_FILE_BYTES {
                    continue;
                }
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                sink.side_file(&entry_name, &kind, content)?;
            }
            EntryKind::Ignored => {}
        }
    }
    Ok(())
}

/// Counts bytes as they are read, for progress reporting.
struct CountingReader<R> {
    inner: R,
    count: Rc<Cell<u64>>,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.set(self.count.get() + n as u64);
        Ok(n)
    }
}

/// Parses the top-level array one conversation at a time. The file is about
/// 150 MB, so it is never held in memory as a whole.
pub fn stream_conversations<R: Read>(
    reader: R,
    file_name: &str,
    total: u64,
    sink: &mut dyn ExportSink,
) -> AppResult<()> {
    let count = Rc::new(Cell::new(0));
    let reader = CountingReader {
        inner: reader,
        count: Rc::clone(&count),
    };
    let mut de = serde_json::Deserializer::from_reader(reader);
    let mut failure = None;
    let seed = ArraySeed {
        sink,
        failure: &mut failure,
        count: &count,
        file_name,
        total,
    };
    let result = seed.deserialize(&mut de);
    // An error from the sink (cancel, database) wins over the generic
    // "aborted" error it was smuggled out through.
    if let Some(err) = failure {
        return Err(err);
    }
    result?;
    de.end()?;
    Ok(())
}

struct ArraySeed<'a> {
    sink: &'a mut dyn ExportSink,
    failure: &'a mut Option<AppError>,
    count: &'a Rc<Cell<u64>>,
    file_name: &'a str,
    total: u64,
}

impl<'de> DeserializeSeed<'de> for ArraySeed<'_> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for ArraySeed<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an array of conversations")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(value) = seq.next_element::<Value>()? {
            if let Err(err) = self.sink.conversation(value) {
                *self.failure = Some(err);
                return Err(de::Error::custom("import aborted"));
            }
            self.sink
                .progress(self.file_name, self.count.get(), self.total);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_zip_entries() {
        assert_eq!(
            classify_entry("conversations.json"),
            EntryKind::Conversations
        );
        assert_eq!(classify_entry("projects/abc.json"), EntryKind::Project);
        assert_eq!(
            classify_entry("design_chats/abc.json"),
            EntryKind::DesignChat
        );
        assert_eq!(classify_entry("login_history.json"), EntryKind::Ignored);
        assert_eq!(classify_entry("users.json"), EntryKind::Ignored);
        assert_eq!(
            classify_entry("projects/nested/abc.json"),
            EntryKind::Ignored
        );
        assert_eq!(
            classify_entry("artifacts/x/artifact.json"),
            EntryKind::Ignored
        );
    }
}
