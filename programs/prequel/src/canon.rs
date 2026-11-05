//! Ticker canon (SPEC §2, decision D-015): validate, uppercase, map confusables, hash.
//!
//! The table comes from `packages/shared/canon/confusables.txt` via `build.rs`; the TypeScript library reads
//! the same file, and `packages/shared/canon/fixtures.tsv` proves both sides agree.

use solana_sha256_hasher::hash;

include!(concat!(env!("OUT_DIR"), "/canon_table.rs"));

pub const MIN_TICKER_LEN: usize = 2;
pub const MAX_TICKER_LEN: usize = 10;

#[derive(Debug, PartialEq, Eq)]
pub enum CanonError {
    /// Length outside 2..=10 bytes.
    BadLength,
    /// Anything other than ASCII letters and digits. Checked before any case mapping.
    BadCharacter,
}

/// Validates the ticker and returns it uppercased (the form shown to users), without confusable mapping.
pub fn display_ticker(ticker: &str) -> Result<Vec<u8>, CanonError> {
    let bytes = ticker.as_bytes();
    if bytes.len() < MIN_TICKER_LEN || bytes.len() > MAX_TICKER_LEN {
        return Err(CanonError::BadLength);
    }
    if !bytes.iter().all(|b| b.is_ascii_alphanumeric()) {
        return Err(CanonError::BadCharacter);
    }
    Ok(bytes.iter().map(|b| b.to_ascii_uppercase()).collect())
}

/// Uppercase, then map confusables: the bytes that are hashed to form the candidate address seed.
pub fn canonical_ticker(ticker: &str) -> Result<Vec<u8>, CanonError> {
    let mut bytes = display_ticker(ticker)?;
    for b in bytes.iter_mut() {
        if let Some((_, to)) = CONFUSABLES.iter().find(|(from, _)| from == b) {
            *b = *to;
        }
    }
    Ok(bytes)
}

/// sha256 of the canonical bytes: the `canon_hash` seed of the Candidate PDA.
pub fn canonical_ticker_hash(ticker: &str) -> Result<[u8; 32], CanonError> {
    Ok(hash(&canonical_ticker(ticker)?).to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Shared with the TypeScript tests: packages/shared/canon/fixtures.tsv (input, canonical or ERR, sha256 hex).
    #[test]
    fn matches_the_shared_fixtures() {
        let fixtures = include_str!("../../../packages/shared/canon/fixtures.tsv");
        let mut checked = 0;
        for line in fixtures
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
        {
            let cols: Vec<&str> = line.split('\t').collect();
            assert_eq!(cols.len(), 3, "bad fixture line: {line:?}");
            let (input, expected, expected_hash) = (cols[0], cols[1], cols[2]);
            match canonical_ticker(input) {
                Ok(canon) => {
                    assert_eq!(
                        String::from_utf8(canon).unwrap(),
                        expected,
                        "canonical form of {input:?}"
                    );
                    assert_eq!(
                        hex(&canonical_ticker_hash(input).unwrap()),
                        expected_hash,
                        "hash of {input:?}"
                    );
                }
                Err(_) => assert_eq!(expected, "ERR", "{input:?} should be accepted"),
            }
            checked += 1;
        }
        assert!(checked >= 30, "fixtures went missing: {checked}");
    }

    #[test]
    fn table_is_frozen_version_one() {
        assert_eq!(CANON_VERSION, 1);
        assert_eq!(CONFUSABLES, [(b'0', b'O'), (b'1', b'I'), (b'5', b'S')]);
    }

    #[test]
    fn display_keeps_digits_and_rejects_non_ascii_before_case_mapping() {
        assert_eq!(display_ticker("b0b").unwrap(), b"B0B");
        assert_eq!(
            display_ticker("\u{00df}\u{00df}"),
            Err(CanonError::BadCharacter)
        );
        assert_eq!(display_ticker("a"), Err(CanonError::BadLength));
        assert_eq!(display_ticker("ABCDEFGHIJK"), Err(CanonError::BadLength));
    }

    #[test]
    fn canonical_form_is_idempotent() {
        for t in ["BOB", "b0b", "S5S", "1234567890", "PEPE"] {
            let once = canonical_ticker(t).unwrap();
            let twice = canonical_ticker(core::str::from_utf8(&once).unwrap()).unwrap();
            assert_eq!(once, twice);
        }
    }
}
