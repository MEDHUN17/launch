//! Generates `canon_table.rs` from the shared confusables table, so the on-chain ticker canon and the
//! TypeScript library (packages/shared) read the same file (SPEC §2, decision D-009/D-015).

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let table = manifest.join("../../packages/shared/canon/confusables.txt");
    println!("cargo:rerun-if-changed={}", table.display());
    let text = fs::read_to_string(&table)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", table.display()));

    let mut version: Option<u8> = None;
    let mut pairs: Vec<(u8, u8)> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["version", v] => version = Some(v.parse().expect("version must be a u8")),
            [from, to] if from.len() == 1 && to.len() == 1 => {
                let (f, t) = (from.as_bytes()[0], to.as_bytes()[0]);
                assert!(f.is_ascii_alphanumeric() && t.is_ascii_alphanumeric(), "line {}: ASCII letters/digits only", n + 1);
                assert!(t.is_ascii_digit() || t.is_ascii_uppercase(), "line {}: target must be A-Z or 0-9", n + 1);
                pairs.push((f.to_ascii_uppercase(), t));
            }
            _ => panic!("{}:{}: expected `version <n>` or `<from> <to>`", table.display(), n + 1),
        }
    }
    let version = version.expect("confusables.txt needs a `version` line");
    for (i, (f, _)) in pairs.iter().enumerate() {
        assert!(pairs.iter().skip(i + 1).all(|(g, _)| g != f), "duplicate source {}", *f as char);
    }
    assert!(pairs.iter().all(|(_, t)| pairs.iter().all(|(f, _)| f != t)), "a target is also a source: the mapping would not be idempotent");

    let entries: Vec<String> = pairs.iter().map(|(f, t)| format!("(b'{}', b'{}')", *f as char, *t as char)).collect();
    let out = format!(
        "pub const CANON_VERSION: u8 = {version};\npub const CONFUSABLES: [(u8, u8); {}] = [{}];\n",
        pairs.len(),
        entries.join(", ")
    );
    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("canon_table.rs");
    fs::write(dest, out).unwrap();
}
