use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

pub type CancelFn<'a> = &'a (dyn Fn() -> bool + Send + Sync);

const COPY_BUF_SIZE: usize = 4 * 1024 * 1024;
const HEADER_PREFIX_SIZE: u64 = 8;

fn check_cancel(cancel: CancelFn) -> io::Result<()> {
    if cancel() {
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
    } else {
        Ok(())
    }
}

fn u32_le(b: &[u8]) -> u32 {
    u32::from_le_bytes(b.try_into().unwrap())
}

fn unpacked_dir_for(asar_path: &Path) -> PathBuf {
    let mut os: OsString = asar_path.as_os_str().to_os_string();
    os.push(".unpacked");
    PathBuf::from(os)
}

pub fn extract_archive(asar_path: &Path, dest: &Path, cancel: CancelFn) -> io::Result<()> {
    let mut f = File::open(asar_path)?;

    let mut prefix = [0u8; 16];
    f.read_exact(&mut prefix)?;
    if u32_le(&prefix[0..4]) != 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid asar header: expected pickle size 4",
        ));
    }
    let pickle2_total = u32_le(&prefix[4..8]) as u64;
    let json_len = u32_le(&prefix[12..16]) as usize;
    let file_data_start = HEADER_PREFIX_SIZE + pickle2_total;

    let mut json_buf = vec![0u8; json_len];
    f.read_exact(&mut json_buf)?;
    let header: Value = serde_json::from_slice(&json_buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let files = header
        .get("files")
        .and_then(|v| v.as_object())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing files object"))?;

    let unpacked_root = unpacked_dir_for(asar_path);
    fs::create_dir_all(dest)?;

    extract_dir(
        files,
        dest,
        &unpacked_root,
        Path::new(""),
        &mut f,
        file_data_start,
        cancel,
    )
}

fn extract_dir<R: Read + Seek>(
    files: &Map<String, Value>,
    out_dir: &Path,
    unpacked_root: &Path,
    rel: &Path,
    body: &mut R,
    file_data_start: u64,
    cancel: CancelFn,
) -> io::Result<()> {
    for (name, entry) in files {
        check_cancel(cancel)?;
        let out_path = out_dir.join(name);
        let next_rel = rel.join(name);

        if let Some(sub) = entry.get("files").and_then(|v| v.as_object()) {
            fs::create_dir_all(&out_path)?;
            extract_dir(
                sub,
                &out_path,
                unpacked_root,
                &next_rel,
                body,
                file_data_start,
                cancel,
            )?;
            continue;
        }
        if entry.get("link").is_some() {
            continue;
        }

        let size = entry.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        let is_unpacked = entry
            .get("unpacked")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if is_unpacked {
            let src = unpacked_root.join(&next_rel);
            fs::copy(&src, &out_path)?;
        } else {
            let offset_str = entry
                .get("offset")
                .and_then(|v| v.as_str())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing offset"))?;
            let offset: u64 = offset_str
                .parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad offset"))?;
            body.seek(SeekFrom::Start(file_data_start + offset))?;
            let mut out = BufWriter::with_capacity(COPY_BUF_SIZE, File::create(&out_path)?);
            copy_exact(body, &mut out, size, cancel)?;
            out.flush()?;
        }
    }
    Ok(())
}

fn copy_exact<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    mut remaining: u64,
    cancel: CancelFn,
) -> io::Result<()> {
    let mut buf = vec![0u8; COPY_BUF_SIZE];
    while remaining > 0 {
        check_cancel(cancel)?;
        let to_read = remaining.min(buf.len() as u64) as usize;
        reader.read_exact(&mut buf[..to_read])?;
        writer.write_all(&buf[..to_read])?;
        remaining -= to_read as u64;
    }
    Ok(())
}

#[derive(Debug)]
struct FileEntry {
    rel_path: PathBuf,
    abs_path: PathBuf,
    size: u64,
    unpacked: bool,
    executable: bool,
}

pub fn create_archive(
    src: &Path,
    asar_path: &Path,
    unpack_patterns: &[&str],
    cancel: CancelFn,
) -> io::Result<()> {
    let mut files = Vec::new();
    collect_files(src, src, unpack_patterns, &mut files, cancel)?;

    let mut offsets = Vec::with_capacity(files.len());
    let mut cursor: u64 = 0;
    for f in &files {
        if f.unpacked {
            offsets.push(None);
        } else {
            offsets.push(Some(cursor));
            cursor += f.size;
        }
    }

    let mut root_files: Map<String, Value> = Map::new();
    for (i, f) in files.iter().enumerate() {
        let mut entry = Map::new();
        entry.insert("size".into(), Value::from(f.size));
        if f.unpacked {
            entry.insert("unpacked".into(), Value::Bool(true));
        } else {
            entry.insert(
                "offset".into(),
                Value::String(offsets[i].unwrap().to_string()),
            );
        }
        if f.executable {
            entry.insert("executable".into(), Value::Bool(true));
        }
        insert_into_tree(&mut root_files, &f.rel_path, Value::Object(entry));
    }
    let header = serde_json::json!({ "files": root_files });

    let json_bytes = serde_json::to_vec(&header)?;
    let json_len = json_bytes.len() as u64;
    let aligned_json = (json_len + 3) & !3u64;
    let padding = (aligned_json - json_len) as usize;
    let pickle2_payload_size = 4 + aligned_json;
    let pickle2_total = 4 + pickle2_payload_size;

    if pickle2_total > u32::MAX as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "asar header larger than 4GiB",
        ));
    }

    if let Some(parent) = asar_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let unpacked_root = unpacked_dir_for(asar_path);
    if unpacked_root.exists() {
        fs::remove_dir_all(&unpacked_root)?;
    }

    let mut out = BufWriter::with_capacity(COPY_BUF_SIZE, File::create(asar_path)?);
    out.write_all(&4u32.to_le_bytes())?;
    out.write_all(&(pickle2_total as u32).to_le_bytes())?;
    out.write_all(&(pickle2_payload_size as u32).to_le_bytes())?;
    out.write_all(&(json_len as u32).to_le_bytes())?;
    out.write_all(&json_bytes)?;
    if padding > 0 {
        out.write_all(&vec![0u8; padding])?;
    }

    for f in &files {
        check_cancel(cancel)?;
        if f.unpacked {
            let dst = unpacked_root.join(&f.rel_path);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&f.abs_path, &dst)?;
        } else {
            let mut reader = File::open(&f.abs_path)?;
            copy_exact(&mut reader, &mut out, f.size, cancel)?;
        }
    }
    out.flush()?;
    Ok(())
}

fn collect_files(
    root: &Path,
    current: &Path,
    unpack_patterns: &[&str],
    out: &mut Vec<FileEntry>,
    cancel: CancelFn,
) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        check_cancel(cancel)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;

        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_files(root, &path, unpack_patterns, out, cancel)?;
        } else if metadata.is_file() {
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            let name = rel
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let is_unpacked = unpack_patterns.iter().any(|p| glob_match(p, &name));

            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let executable = false;

            out.push(FileEntry {
                rel_path: rel,
                abs_path: path,
                size: metadata.len(),
                unpacked: is_unpacked,
                executable,
            });
        }
    }
    Ok(())
}

fn insert_into_tree(root: &mut Map<String, Value>, rel_path: &Path, leaf: Value) {
    let components: Vec<String> = rel_path
        .components()
        .filter_map(|c| {
            if let Component::Normal(s) = c {
                s.to_str().map(String::from)
            } else {
                None
            }
        })
        .collect();

    fn go(map: &mut Map<String, Value>, parts: &[String], leaf: Value) {
        if parts.is_empty() {
            return;
        }
        if parts.len() == 1 {
            map.insert(parts[0].clone(), leaf);
            return;
        }
        let entry = map
            .entry(parts[0].clone())
            .or_insert_with(|| serde_json::json!({ "files": {} }));
        if let Value::Object(obj) = entry {
            if !obj.contains_key("files") {
                obj.insert("files".into(), Value::Object(Map::new()));
            }
            if let Some(Value::Object(sub)) = obj.get_mut("files") {
                go(sub, &parts[1..], leaf);
            }
        }
    }

    go(root, &components, leaf);
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star_p, mut star_t): (Option<usize>, usize) = (None, 0);

    while ti < t.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star_p = Some(pi);
            star_t = ti;
            pi += 1;
        } else if let Some(sp) = star_p {
            pi = sp + 1;
            star_t += 1;
            ti = star_t;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn glob_basic() {
        assert!(glob_match("*.node", "steamworks.node"));
        assert!(glob_match("*.node", "a.node"));
        assert!(!glob_match("*.node", "foo.txt"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("a*b", "aXb"));
        assert!(glob_match("a*b", "aXXXb"));
        assert!(!glob_match("a*b", "aXc"));
        assert!(glob_match("foo.node", "foo.node"));
    }
}
