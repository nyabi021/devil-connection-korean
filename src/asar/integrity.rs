//! SHA256 integrity metadata matching Electron's asar format:
//! - `hash`: SHA256 of the entire file, hex-encoded.
//! - `blocks[i]`: SHA256 of the i-th 4 MiB chunk, hex-encoded.
//! - For single-block files, `hash == blocks[0]`.

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::BLOCK_SIZE;

pub(crate) struct Integrity {
    pub hash: String,
    pub blocks: Vec<String>,
}

pub(crate) struct IntegrityHasher {
    whole: Sha256,
    block: Sha256,
    block_filled: usize,
    blocks: Vec<String>,
}

impl IntegrityHasher {
    pub fn new() -> Self {
        Self {
            whole: Sha256::new(),
            block: Sha256::new(),
            block_filled: 0,
            blocks: Vec::new(),
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.whole.update(data);
        while !data.is_empty() {
            let take = (BLOCK_SIZE - self.block_filled).min(data.len());
            self.block.update(&data[..take]);
            self.block_filled += take;
            data = &data[take..];
            if self.block_filled == BLOCK_SIZE {
                let digest = std::mem::replace(&mut self.block, Sha256::new()).finalize();
                self.blocks.push(hex(&digest));
                self.block_filled = 0;
            }
        }
    }

    pub fn finalize(mut self) -> Integrity {
        if self.block_filled > 0 || self.blocks.is_empty() {
            let digest = self.block.finalize();
            self.blocks.push(hex(&digest));
        }
        let hash = hex(&self.whole.finalize());
        Integrity {
            hash,
            blocks: self.blocks,
        }
    }
}

pub(crate) fn to_json(integrity: &Integrity) -> Value {
    let mut obj = Map::new();
    obj.insert("algorithm".into(), json!("SHA256"));
    obj.insert("hash".into(), json!(integrity.hash));
    obj.insert("blockSize".into(), json!(BLOCK_SIZE));
    obj.insert("blocks".into(), json!(integrity.blocks));
    Value::Object(obj)
}

/// Build a placeholder integrity JSON object with the correct shape and byte
/// length so the packer can reserve space in the header, then overwrite the
/// hashes in place after streaming the files. `size` is the file size in bytes.
pub(crate) fn placeholder(size: u64) -> Value {
    let block_count = if size == 0 {
        1
    } else {
        ((size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64) as usize
    };
    let placeholder_hash = "0".repeat(64);
    let blocks: Vec<String> = (0..block_count).map(|_| placeholder_hash.clone()).collect();
    to_json(&Integrity {
        hash: placeholder_hash,
        blocks,
    })
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(nibble(b >> 4));
        s.push(nibble(b & 0x0f));
    }
    s
}

fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + n - 10) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_block_hash_matches_whole_file_hash() {
        let data = b"hello asar";
        let mut h = IntegrityHasher::new();
        h.update(data);
        let i = h.finalize();
        assert_eq!(i.blocks.len(), 1);
        assert_eq!(i.hash, i.blocks[0]);
    }

    #[test]
    fn multi_block_split() {
        let data = vec![0xabu8; BLOCK_SIZE + 100];
        let mut h = IntegrityHasher::new();
        h.update(&data);
        let i = h.finalize();
        assert_eq!(i.blocks.len(), 2);
        assert_ne!(i.hash, i.blocks[0]);
    }

    #[test]
    fn placeholder_byte_length_matches_final() {
        let size = BLOCK_SIZE as u64 * 3 + 42;
        let ph = placeholder(size);
        let real = to_json(&Integrity {
            hash: "a".repeat(64),
            blocks: vec!["b".repeat(64); 4],
        });
        let ph_bytes = serde_json::to_vec(&ph).unwrap();
        let real_bytes = serde_json::to_vec(&real).unwrap();
        assert_eq!(ph_bytes.len(), real_bytes.len());
    }
}
