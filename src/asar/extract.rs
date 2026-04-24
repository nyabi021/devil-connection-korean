//! Extract an asar archive into a destination directory, including any files
//! stored in the sibling `<archive>.unpacked/` tree.

use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use super::{AsarError, Progress, ProgressEvent, header};

const COPY_BUF: usize = 1024 * 1024;

/// Parse just the header of an asar archive. Useful for diagnostics without
/// touching the multi-gigabyte body.
#[allow(dead_code)]
pub fn read_header(archive: &Path) -> super::Result<(serde_json::Value, u64)> {
    let file = File::open(archive).map_err(|e| AsarError::io(archive, e))?;
    let mut reader = BufReader::new(file);
    let h = header::read(&mut reader)?;
    Ok((h.json, h.data_offset))
}

pub fn extract(
    archive: &Path,
    dest: &Path,
    progress: &mut Progress<'_>,
) -> super::Result<()> {
    let file = File::open(archive).map_err(|e| AsarError::io(archive, e))?;
    let mut reader = BufReader::new(file);
    let h = header::read(&mut reader)?;

    let unpacked_dir = unpacked_root(archive);
    let entries = collect_entries(&h.json)?;
    let total_bytes = entries.iter().map(|e| e.size).sum();
    let total_files = entries.len() as u64;

    progress.emit(ProgressEvent::Started {
        total_bytes,
        total_files,
    });

    fs::create_dir_all(dest).map_err(|e| AsarError::io(dest, e))?;

    for entry in &entries {
        progress.check_cancel()?;
        let rel = entry.as_rel_path();
        let dest_path = dest.join(&rel);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|e| AsarError::io(parent, e))?;
        }
        progress.emit(ProgressEvent::FileStarted {
            path: entry.path.clone(),
            bytes: entry.size,
        });

        match &entry.kind {
            EntryKind::Packed { offset } => {
                reader
                    .seek(SeekFrom::Start(h.data_offset + offset))
                    .map_err(|e| AsarError::io(archive, e))?;
                let out = File::create(&dest_path)
                    .map_err(|e| AsarError::io(&dest_path, e))?;
                copy_exact(&mut reader, &mut BufWriter::new(out), entry.size, progress)?;
            }
            EntryKind::Unpacked => {
                let src = unpacked_dir.join(&rel);
                let mut input =
                    File::open(&src).map_err(|e| AsarError::io(&src, e))?;
                let out = File::create(&dest_path)
                    .map_err(|e| AsarError::io(&dest_path, e))?;
                copy_exact(&mut input, &mut BufWriter::new(out), entry.size, progress)?;
            }
            EntryKind::Link { target } => {
                // Windows symlink creation requires privileges; write a plain
                // file with the link target as content so the patcher can still
                // round-trip the archive if such entries ever appear.
                let out = File::create(&dest_path)
                    .map_err(|e| AsarError::io(&dest_path, e))?;
                BufWriter::new(out)
                    .write_all(target.as_bytes())
                    .map_err(|e| AsarError::io(&dest_path, e))?;
            }
        }
        progress.emit(ProgressEvent::FileFinished);
    }

    progress.emit(ProgressEvent::Finished);
    Ok(())
}

pub(crate) fn unpacked_root(archive: &Path) -> PathBuf {
    let name = archive
        .file_name()
        .map(|s| {
            let mut n = s.to_os_string();
            n.push(".unpacked");
            n
        })
        .unwrap_or_else(|| std::ffi::OsString::from("app.asar.unpacked"));
    archive
        .parent()
        .map(|p| p.join(&name))
        .unwrap_or_else(|| PathBuf::from(name))
}

enum EntryKind {
    Packed { offset: u64 },
    Unpacked,
    Link { target: String },
}

struct Entry {
    path: String,
    size: u64,
    kind: EntryKind,
}

impl Entry {
    fn as_rel_path(&self) -> PathBuf {
        PathBuf::from(self.path.replace('/', std::path::MAIN_SEPARATOR_STR))
    }
}

fn collect_entries(json: &Value) -> super::Result<Vec<Entry>> {
    let mut out = Vec::new();
    walk(json, String::new(), &mut out)?;
    Ok(out)
}

fn walk(node: &Value, prefix: String, out: &mut Vec<Entry>) -> super::Result<()> {
    let obj = node.as_object().ok_or_else(|| AsarError::InvalidEntry {
        path: prefix.clone(),
        reason: "expected object".into(),
    })?;
    let files = obj
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| AsarError::InvalidEntry {
            path: prefix.clone(),
            reason: "missing 'files'".into(),
        })?;
    for (name, child) in files {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let child_obj = child.as_object().ok_or_else(|| AsarError::InvalidEntry {
            path: path.clone(),
            reason: "expected object".into(),
        })?;
        if child_obj.contains_key("files") {
            walk(child, path, out)?;
        } else if let Some(link) = child_obj.get("link").and_then(Value::as_str) {
            out.push(Entry {
                path,
                size: link.len() as u64,
                kind: EntryKind::Link {
                    target: link.to_string(),
                },
            });
        } else {
            let size = child_obj
                .get("size")
                .and_then(Value::as_u64)
                .ok_or_else(|| AsarError::InvalidEntry {
                    path: path.clone(),
                    reason: "missing 'size'".into(),
                })?;
            let unpacked = child_obj
                .get("unpacked")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let kind = if unpacked {
                EntryKind::Unpacked
            } else {
                let offset = parse_offset(child_obj.get("offset"), &path)?;
                EntryKind::Packed { offset }
            };
            out.push(Entry { path, size, kind });
        }
    }
    Ok(())
}

fn parse_offset(value: Option<&Value>, path: &str) -> super::Result<u64> {
    let v = value.ok_or_else(|| AsarError::InvalidEntry {
        path: path.into(),
        reason: "missing 'offset'".into(),
    })?;
    match v {
        Value::String(s) => s.parse::<u64>().map_err(|_| AsarError::InvalidEntry {
            path: path.into(),
            reason: format!("offset '{s}' is not a valid u64"),
        }),
        Value::Number(n) => n.as_u64().ok_or_else(|| AsarError::InvalidEntry {
            path: path.into(),
            reason: "offset number out of u64 range".into(),
        }),
        _ => Err(AsarError::InvalidEntry {
            path: path.into(),
            reason: "offset must be string or number".into(),
        }),
    }
}

fn copy_exact<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    mut remaining: u64,
    progress: &mut Progress<'_>,
) -> super::Result<()> {
    let mut buf = vec![0u8; COPY_BUF];
    while remaining > 0 {
        progress.check_cancel()?;
        let want = remaining.min(buf.len() as u64) as usize;
        reader
            .read_exact(&mut buf[..want])
            .map_err(|e| AsarError::io(Path::new("<archive>"), e))?;
        writer
            .write_all(&buf[..want])
            .map_err(|e| AsarError::io(Path::new("<output>"), e))?;
        remaining -= want as u64;
        progress.emit(ProgressEvent::Bytes { delta: want as u64 });
    }
    writer
        .flush()
        .map_err(|e| AsarError::io(Path::new("<output>"), e))?;
    Ok(())
}
