//! Read and write the asar header.
//!
//! Binary layout (little-endian, Chromium Pickle-in-Pickle):
//! ```text
//! [u32] 4                          // outer pickle payload size (always 4)
//! [u32] header_buf_len             // total byte length of inner pickle
//! [u32] header_buf_len - 4         // inner pickle payload size
//! [u32] json_len                   // JSON string byte length
//! [json_len bytes] JSON            // the directory tree
//! [pad bytes] 0x00 * ((4 - json_len % 4) & 3)  // align to 4
//! [file data ...]
//! ```
//! File data offsets in the JSON are relative to the start of the file data
//! region, which begins at `8 + header_buf_len`.

use serde_json::Value;
use std::io::Read;

use super::AsarError;

pub(crate) struct Header {
    pub(crate) json: Value,
    pub(crate) data_offset: u64,
}

pub(crate) fn read<R: Read>(reader: &mut R) -> super::Result<Header> {
    let outer_size = read_u32(reader)?;
    if outer_size != 4 {
        return Err(AsarError::MalformedHeader(format!(
            "outer pickle size must be 4, got {outer_size}"
        )));
    }
    let header_buf_len = read_u32(reader)? as u64;
    let inner_size = read_u32(reader)? as u64;
    if inner_size + 4 != header_buf_len {
        return Err(AsarError::MalformedHeader(format!(
            "inner pickle size mismatch: expected {}, got {inner_size}",
            header_buf_len - 4
        )));
    }
    let json_len = read_u32(reader)? as usize;
    let pad_len = (4 - json_len % 4) & 3;
    if (4 + json_len + pad_len) as u64 != inner_size {
        return Err(AsarError::MalformedHeader(format!(
            "json length + padding ({} + {}) does not match inner size {inner_size}",
            json_len, pad_len
        )));
    }

    let mut buf = vec![0u8; json_len];
    reader.read_exact(&mut buf).map_err(io_other)?;
    let mut pad = [0u8; 3];
    if pad_len > 0 {
        reader
            .read_exact(&mut pad[..pad_len])
            .map_err(io_other)?;
    }

    let json: Value = serde_json::from_slice(&buf)?;
    let data_offset = 8 + header_buf_len;

    Ok(Header { json, data_offset })
}

pub(crate) fn serialize(json: &Value) -> super::Result<(Vec<u8>, u64)> {
    let json_bytes = serde_json::to_vec(json)?;
    let json_len = json_bytes.len();
    let pad_len = (4 - json_len % 4) & 3;
    let inner_size = (4 + json_len + pad_len) as u32;
    let header_buf_len = inner_size + 4;

    let mut out = Vec::with_capacity(16 + json_len + pad_len);
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&header_buf_len.to_le_bytes());
    out.extend_from_slice(&inner_size.to_le_bytes());
    out.extend_from_slice(&(json_len as u32).to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend(std::iter::repeat_n(0u8, pad_len));

    let data_offset = 8 + header_buf_len as u64;
    Ok((out, data_offset))
}

fn read_u32<R: Read>(reader: &mut R) -> super::Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(io_other)?;
    Ok(u32::from_le_bytes(buf))
}

fn io_other(e: std::io::Error) -> AsarError {
    AsarError::Io {
        path: std::path::PathBuf::new(),
        source: e,
    }
}
