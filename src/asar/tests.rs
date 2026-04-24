#![cfg(test)]

use super::{PackOptions, Progress, extract, pack};
use std::fs;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("asar-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &std::path::Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn pack_then_extract_matches_source() {
    let root = tmp("basic");
    let src = root.join("src");
    let archive = root.join("out.asar");
    let extracted = root.join("ext");

    write(&src.join("package.json"), b"{\"name\":\"demo\"}\n");
    write(&src.join("lib/index.js"), b"console.log('hello')\n");
    write(
        &src.join("lib/nested/data.bin"),
        &vec![0xabu8; super::BLOCK_SIZE + 123],
    );
    write(&src.join("native/hello.node"), b"\x7fELF native stub");
    write(&src.join("native/libextra.so"), b"fake so");

    let mut prog = Progress::new();
    pack(
        &src,
        &archive,
        &PackOptions {
            unpack: Some("*.node".into()),
        },
        &mut prog,
    )
    .unwrap();

    assert!(archive.is_file(), "archive was not created");
    let sidecar = root.join("out.asar.unpacked/native/hello.node");
    assert!(sidecar.is_file(), "native .node should be in the sidecar");
    let inlined = root.join("out.asar.unpacked/native/libextra.so");
    assert!(!inlined.exists(), ".so should be packed, not in sidecar");

    extract(&archive, &extracted, &mut prog).unwrap();
    for rel in [
        "package.json",
        "lib/index.js",
        "lib/nested/data.bin",
        "native/hello.node",
        "native/libextra.so",
    ] {
        let a = fs::read(src.join(rel)).unwrap();
        let b = fs::read(extracted.join(rel)).unwrap();
        assert_eq!(a, b, "mismatch at {rel}");
    }

    let _ = fs::remove_dir_all(&root);
}
