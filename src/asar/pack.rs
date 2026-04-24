//! Pack a directory into an asar archive, honoring a basename glob that
//! selects which files are stored in the `.unpacked` sidecar tree instead of
//! inside the archive.
//!
//! Strategy (single I/O pass for the large archive body):
//! 1. Walk the source tree, collect all files with sizes. Sort deterministically.
//! 2. Build the header JSON with placeholder integrity hashes of the correct
//!    byte length. This fixes the header size and therefore every file offset.
//! 3. Serialize and write the placeholder header.
//! 4. Stream each file exactly once: hash while writing to either the archive
//!    body (packed) or the `.unpacked` sidecar (unpacked), updating the JSON
//!    tree with real hashes as we go.
//! 5. Seek back to the start of the archive and rewrite the header; it has
//!    the same byte length as the placeholder so offsets stay valid.

use globset::{Glob, GlobMatcher};
use serde_json::{Map, Value, json};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::extract::unpacked_root;
use super::integrity::{IntegrityHasher, placeholder, to_json};
use super::{AsarError, Progress, ProgressEvent, header};

const STREAM_BUF: usize = 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct PackOptions {
    /// Basename glob. A file whose name matches is stored in the `.unpacked`
    /// sidecar tree rather than inside the archive. The match is applied to
    /// the file's basename only, matching the behaviour of the Python `asar`
    /// package used by the original patcher.
    pub unpack: Option<String>,
}

pub fn pack(
    src: &Path,
    out: &Path,
    opts: &PackOptions,
    progress: &mut Progress<'_>,
) -> super::Result<()> {
    let matcher = opts
        .unpack
        .as_deref()
        .map(Glob::new)
        .transpose()?
        .map(|g| g.compile_matcher());

    let files = collect_files(src, matcher.as_ref())?;
    let total_bytes: u64 = files.iter().map(|f| f.size).sum();
    let total_files = files.len() as u64;

    progress.emit(ProgressEvent::Started {
        total_bytes,
        total_files,
    });

    let mut tree = json!({ "files": Value::Object(Map::new()) });
    let mut offset: u64 = 0;
    for file in &files {
        let mut node = Map::new();
        node.insert("size".into(), json!(file.size));
        if file.unpacked {
            node.insert("unpacked".into(), json!(true));
        } else {
            node.insert("offset".into(), json!(offset.to_string()));
            offset += file.size;
        }
        node.insert("integrity".into(), placeholder(file.size));
        insert_at_path(&mut tree, &file.components, Value::Object(node))?;
    }

    let (header_bytes, _data_offset) = header::serialize(&tree)?;
    let header_len = header_bytes.len();

    let unpacked_dir = unpacked_root(out);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| AsarError::io(parent, e))?;
    }

    let out_file = File::create(out).map_err(|e| AsarError::io(out, e))?;
    let mut writer = BufWriter::with_capacity(STREAM_BUF, out_file);
    writer
        .write_all(&header_bytes)
        .map_err(|e| AsarError::io(out, e))?;

    for file in &files {
        progress.check_cancel()?;
        let src_path = src.join(&file.rel);
        progress.emit(ProgressEvent::FileStarted {
            path: file.components.join("/"),
            bytes: file.size,
        });

        let integrity = if file.unpacked {
            let sidecar = unpacked_dir.join(&file.rel);
            if let Some(parent) = sidecar.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| AsarError::io(parent, e))?;
            }
            let dst = File::create(&sidecar)
                .map_err(|e| AsarError::io(&sidecar, e))?;
            stream_and_hash(&src_path, &mut BufWriter::new(dst), progress)?
        } else {
            stream_and_hash(&src_path, &mut writer, progress)?
        };

        patch_integrity(&mut tree, &file.components, &integrity)?;
        progress.emit(ProgressEvent::FileFinished);
    }

    writer.flush().map_err(|e| AsarError::io(out, e))?;
    drop(writer);

    let (final_header, _) = header::serialize(&tree)?;
    if final_header.len() != header_len {
        return Err(AsarError::MalformedHeader(format!(
            "header byte length changed after patching integrity (was {header_len}, now {})",
            final_header.len()
        )));
    }
    let mut f = OpenOptions::new()
        .write(true)
        .open(out)
        .map_err(|e| AsarError::io(out, e))?;
    f.seek(SeekFrom::Start(0))
        .map_err(|e| AsarError::io(out, e))?;
    f.write_all(&final_header)
        .map_err(|e| AsarError::io(out, e))?;
    f.flush().map_err(|e| AsarError::io(out, e))?;

    progress.emit(ProgressEvent::Finished);
    Ok(())
}

impl From<std::io::Error> for AsarError {
    fn from(e: std::io::Error) -> Self {
        AsarError::Io {
            path: PathBuf::new(),
            source: e,
        }
    }
}

struct FileEntry {
    rel: PathBuf,
    components: Vec<String>,
    size: u64,
    unpacked: bool,
}

fn collect_files(src: &Path, matcher: Option<&GlobMatcher>) -> super::Result<Vec<FileEntry>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(src)
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .into_iter()
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(src)
            .map_err(|_| AsarError::InvalidEntry {
                path: entry.path().display().to_string(),
                reason: "entry outside source root".into(),
            })?
            .to_path_buf();
        let components: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let basename = components
            .last()
            .cloned()
            .unwrap_or_default();
        let size = entry.metadata()?.len();
        let unpacked = matcher
            .map(|m| m.is_match(&basename))
            .unwrap_or(false);
        files.push(FileEntry {
            rel,
            components,
            size,
            unpacked,
        });
    }
    Ok(files)
}

fn insert_at_path(
    tree: &mut Value,
    components: &[String],
    leaf: Value,
) -> super::Result<()> {
    let mut cursor = tree
        .as_object_mut()
        .and_then(|o| o.get_mut("files"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| AsarError::MalformedHeader("root 'files' missing".into()))?;

    for (i, name) in components.iter().enumerate() {
        let is_leaf = i == components.len() - 1;
        if is_leaf {
            cursor.insert(name.clone(), leaf);
            return Ok(());
        }
        if !cursor.contains_key(name) {
            let mut dir = Map::new();
            dir.insert("files".into(), Value::Object(Map::new()));
            cursor.insert(name.clone(), Value::Object(dir));
        }
        cursor = cursor
            .get_mut(name)
            .and_then(Value::as_object_mut)
            .and_then(|o| o.get_mut("files"))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| AsarError::MalformedHeader(format!(
                "intermediate node '{name}' is not a directory"
            )))?;
    }
    Ok(())
}

fn patch_integrity(
    tree: &mut Value,
    components: &[String],
    integrity: &super::integrity::Integrity,
) -> super::Result<()> {
    let mut cursor = tree
        .as_object_mut()
        .and_then(|o| o.get_mut("files"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| AsarError::MalformedHeader("root 'files' missing".into()))?;

    for (i, name) in components.iter().enumerate() {
        let is_leaf = i == components.len() - 1;
        if is_leaf {
            let node = cursor
                .get_mut(name)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| AsarError::MalformedHeader(format!(
                    "leaf '{name}' is not an object"
                )))?;
            node.insert("integrity".into(), to_json(integrity));
            return Ok(());
        }
        cursor = cursor
            .get_mut(name)
            .and_then(Value::as_object_mut)
            .and_then(|o| o.get_mut("files"))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| AsarError::MalformedHeader(format!(
                "intermediate node '{name}' missing"
            )))?;
    }
    Ok(())
}

fn stream_and_hash<W: Write>(
    src: &Path,
    writer: &mut W,
    progress: &mut Progress<'_>,
) -> super::Result<super::integrity::Integrity> {
    let file = File::open(src).map_err(|e| AsarError::io(src, e))?;
    let mut reader = BufReader::with_capacity(STREAM_BUF, file);
    let mut hasher = IntegrityHasher::new();
    let mut buf = vec![0u8; STREAM_BUF];
    loop {
        progress.check_cancel()?;
        let n = reader.read(&mut buf).map_err(|e| AsarError::io(src, e))?;
        if n == 0 {
            break;
        }
        let chunk = &buf[..n];
        hasher.update(chunk);
        writer
            .write_all(chunk)
            .map_err(|e| AsarError::io(src, e))?;
        progress.emit(ProgressEvent::Bytes { delta: n as u64 });
    }
    writer.flush().map_err(|e| AsarError::io(src, e))?;
    Ok(hasher.finalize())
}
