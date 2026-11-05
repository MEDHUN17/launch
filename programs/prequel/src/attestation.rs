//! Canonicalizer attestation (decision D-014, SPEC §2 "Attestation").
//!
//! When an event requires attestation, `propose_candidate` must be preceded, in the same transaction and
//! immediately before it, by a native Ed25519 precompile instruction with exactly one signature over the message
//! below, made by the configured canonicalizer key. The runtime verifies the signature; this module checks that
//! the instruction says what we need: right key, right bytes, nothing pointing at other instructions.

use crate::error::PrequelError;
use anchor_lang::prelude::*;
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};

pub const ED25519_PROGRAM_ID: Pubkey = pubkey!("Ed25519SigVerify111111111111111111111111111");
pub const INSTRUCTIONS_SYSVAR_ID: Pubkey = pubkey!("Sysvar1nstructions1111111111111111111111111");
/// 2-byte header (count, padding) + one 14-byte offsets record.
const HEADER_LEN: usize = 2 + 14;
/// The only value we accept for the three `*_instruction_index` fields: "this instruction".
const SAME_INSTRUCTION: u16 = u16::MAX;

/// Everything the canonicalizer signs. Layout (251 bytes): domain tag (18) | program id | config | event |
/// outcome (1) | canon hash | content hash | image hash | proposer | expiry (i64 LE).
pub struct Attested<'a> {
    pub program_id: &'a Pubkey,
    pub config: &'a Pubkey,
    pub event: &'a Pubkey,
    pub outcome: u8,
    pub canon_hash: &'a [u8; 32],
    /// sha256(name || 0x00 || display ticker || 0x00 || uri)
    pub content_hash: &'a [u8; 32],
    pub image_hash: &'a [u8; 32],
    pub proposer: &'a Pubkey,
    pub expiry: i64,
}

pub const MESSAGE_LEN: usize = 18 + 32 * 3 + 1 + 32 * 4 + 8;

pub fn content_hash(name: &str, display_ticker: &str, uri: &str) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        name.as_bytes(),
        &[0],
        display_ticker.as_bytes(),
        &[0],
        uri.as_bytes(),
    ])
    .to_bytes()
}

pub fn expected_message(a: &Attested) -> Vec<u8> {
    let mut m = Vec::with_capacity(MESSAGE_LEN);
    m.extend_from_slice(crate::constants::ATTESTATION_DOMAIN);
    m.extend_from_slice(a.program_id.as_ref());
    m.extend_from_slice(a.config.as_ref());
    m.extend_from_slice(a.event.as_ref());
    m.push(a.outcome);
    m.extend_from_slice(a.canon_hash);
    m.extend_from_slice(a.content_hash);
    m.extend_from_slice(a.image_hash);
    m.extend_from_slice(a.proposer.as_ref());
    m.extend_from_slice(&a.expiry.to_le_bytes());
    debug_assert_eq!(m.len(), MESSAGE_LEN);
    m
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedEd25519<'a> {
    pub pubkey: &'a [u8],
    pub message: &'a [u8],
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

/// Structural checks on the data of an Ed25519 precompile instruction: exactly one signature, every offset
/// pointing into this same instruction, every slice inside the data.
pub fn parse_ed25519(data: &[u8]) -> core::result::Result<ParsedEd25519<'_>, PrequelError> {
    use PrequelError::AttestationInvalid as Bad;
    if data.len() < HEADER_LEN || data[0] != 1 {
        return Err(Bad);
    }
    // offsets record: signature_offset, signature_ix, public_key_offset, public_key_ix, message_offset, message_size, message_ix
    let at = |i: usize| u16_at(data, 2 + 2 * i).ok_or(Bad);
    let (sig_off, sig_ix, pk_off, pk_ix, msg_off, msg_len, msg_ix) =
        (at(0)?, at(1)?, at(2)?, at(3)?, at(4)?, at(5)?, at(6)?);
    if sig_ix != SAME_INSTRUCTION || pk_ix != SAME_INSTRUCTION || msg_ix != SAME_INSTRUCTION {
        return Err(Bad);
    }
    let (pk_off, msg_off, msg_len, sig_off) = (
        pk_off as usize,
        msg_off as usize,
        msg_len as usize,
        sig_off as usize,
    );
    data.get(sig_off..sig_off + 64).ok_or(Bad)?;
    let pubkey = data.get(pk_off..pk_off + 32).ok_or(Bad)?;
    let message = data.get(msg_off..msg_off + msg_len).ok_or(Bad)?;
    Ok(ParsedEd25519 { pubkey, message })
}

/// Checks the instruction right before the current one against the canonicalizer key and the exact expected
/// message. `now <= expiry` is required.
pub fn verify(
    instructions_sysvar: &AccountInfo,
    canonicalizer: &Pubkey,
    attested: &Attested,
    now: i64,
) -> Result<()> {
    require!(now <= attested.expiry, PrequelError::AttestationExpired);
    let current = load_current_index_checked(instructions_sysvar)
        .map_err(|_| error!(PrequelError::AttestationMissing))? as usize;
    require!(current > 0, PrequelError::AttestationMissing);
    let previous = load_instruction_at_checked(current - 1, instructions_sysvar)
        .map_err(|_| error!(PrequelError::AttestationMissing))?;
    require!(
        previous.program_id == ED25519_PROGRAM_ID,
        PrequelError::AttestationMissing
    );
    let parsed = parse_ed25519(&previous.data)?;
    require!(
        parsed.pubkey == canonicalizer.as_ref(),
        PrequelError::AttestationInvalid
    );
    require!(
        parsed.message == expected_message(attested).as_slice(),
        PrequelError::AttestationInvalid
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Pubkey, Pubkey, Pubkey, Pubkey, [u8; 32]) {
        (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            [9; 32],
        )
    }

    /// A well-formed precompile instruction body for `pubkey` and `message` (signature bytes are dummy).
    fn ix_data(pubkey: &[u8], message: &[u8], idx: [u16; 3]) -> Vec<u8> {
        let mut d = vec![1u8, 0];
        for v in [48u16, idx[0], 16, idx[1], 112, message.len() as u16, idx[2]] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(pubkey);
        d.extend_from_slice(&[7u8; 64]);
        d.extend_from_slice(message);
        d
    }

    #[test]
    fn message_has_the_documented_layout_and_binds_every_field() {
        let (program, config, event, proposer, h) = sample();
        let base = Attested {
            program_id: &program,
            config: &config,
            event: &event,
            outcome: 1,
            canon_hash: &h,
            content_hash: &h,
            image_hash: &h,
            proposer: &proposer,
            expiry: 1_234,
        };
        let m = expected_message(&base);
        assert_eq!(m.len(), MESSAGE_LEN);
        assert_eq!(MESSAGE_LEN, 251);
        assert_eq!(&m[..18], b"PREQUEL-PROPOSE-V1");
        assert_eq!(&m[18..50], program.as_ref());
        assert_eq!(m[18 + 96], 1, "outcome byte");
        assert_eq!(&m[m.len() - 8..], &1_234i64.to_le_bytes());
        // changing any one input changes the bytes
        let other = Pubkey::new_unique();
        let h2 = [8u8; 32];
        for variant in [
            Attested {
                program_id: &other,
                ..base
            },
            Attested {
                config: &other,
                ..base
            },
            Attested {
                event: &other,
                ..base
            },
            Attested { outcome: 0, ..base },
            Attested {
                canon_hash: &h2,
                ..base
            },
            Attested {
                content_hash: &h2,
                ..base
            },
            Attested {
                image_hash: &h2,
                ..base
            },
            Attested {
                proposer: &other,
                ..base
            },
            Attested {
                expiry: 1_235,
                ..base
            },
        ] {
            assert_ne!(expected_message(&variant), m);
        }
    }

    #[test]
    fn content_hash_separates_the_three_fields() {
        assert_ne!(
            content_hash("ab", "C", "u"),
            content_hash("a", "bC", "u"),
            "name/ticker boundary"
        );
        assert_ne!(
            content_hash("a", "bc", "u"),
            content_hash("a", "b", "cu"),
            "ticker/uri boundary"
        );
        assert_eq!(content_hash("n", "T", "u"), content_hash("n", "T", "u"));
    }

    #[test]
    fn parses_a_well_formed_instruction() {
        let msg = [5u8; 40];
        let d = ix_data(&[3u8; 32], &msg, [u16::MAX; 3]);
        let p = parse_ed25519(&d).unwrap();
        assert_eq!(p.pubkey, &[3u8; 32]);
        assert_eq!(p.message, &msg);
    }

    #[test]
    fn rejects_malformed_or_redirected_instructions() {
        let msg = [5u8; 40];
        let good = ix_data(&[3u8; 32], &msg, [u16::MAX; 3]);
        // any offset pointing at another instruction
        for i in 0..3 {
            let mut idx = [u16::MAX; 3];
            idx[i] = 0;
            assert!(
                parse_ed25519(&ix_data(&[3u8; 32], &msg, idx)).is_err(),
                "index field {i}"
            );
        }
        // not exactly one signature
        for count in [0u8, 2, 255] {
            let mut d = good.clone();
            d[0] = count;
            assert!(parse_ed25519(&d).is_err(), "signature count {count}");
        }
        // truncated at every length shorter than the full body
        for n in 0..good.len() {
            assert!(parse_ed25519(&good[..n]).is_err(), "truncated to {n}");
        }
        // message length pointing past the end
        let mut d = good.clone();
        d[12..14].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(parse_ed25519(&d).is_err());
    }
}
