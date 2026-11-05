//! Attestation tests that need the real Ed25519 precompile (second review, L3 and the weak two-signature test).
//! The byte-level known answers live in a unit test in src/attestation.rs, shared with the TypeScript signer through
//! packages/shared/canon/attestation-vectors.tsv; the tests here prove that the program feeds that code the right
//! inputs and rejects the layouts it must.
mod common;

use anchor_lang::solana_program::instruction::Instruction;
use common::ix::*;
use common::*;
use prequel::{
    attestation::{content_hash, signed_digest, Attested},
    error::PrequelError as E,
    Candidate, ProposeArgs,
};
use solana_keypair::Keypair;
use solana_signer::Signer;

fn attested_env() -> Env {
    let mut env = Env::new();
    env.init_config_with(|p| p.require_attestation = true);
    env.create_default_event("KXNBA-A");
    env
}

fn send_with(
    env: &mut Env,
    w: &Keypair,
    pre: Vec<Instruction>,
    args: &ProposeArgs,
) -> Result<Sent, Failed> {
    let mut ixs = pre;
    ixs.push(propose(&w.pubkey(), 0, &env.treasury, args));
    env.send(&ixs, &[w])
}

/// The digest for `args` with an explicit ticker form inside the content hash.
fn digest_with_ticker(
    args: &ProposeArgs,
    proposer: &anchor_lang::prelude::Pubkey,
    form: &str,
) -> Vec<u8> {
    signed_digest(&Attested {
        program_id: &program_id(),
        config: &config_pda(),
        event: &event_pda(0),
        outcome: args.outcome,
        canon_hash: &args.canon_hash,
        content_hash: &content_hash(&args.name, form, &args.uri),
        image_hash: &args.image_hash,
        proposer,
        expiry: args.attestation_expiry,
    })
    .to_vec()
}

/// A precompile instruction carrying TWO valid signatures by `signer` over `message`, with every offset pointing
/// into the instruction itself. The runtime accepts it; only the program's own "exactly one signature" rule refuses it.
fn two_signature_ix(signer: &Keypair, message: &[u8]) -> Instruction {
    let sig = signer.sign_message(message);
    let header = 2 + 14 * 2;
    let record = 32 + 64 + message.len();
    let mut data = vec![2u8, 0u8];
    for k in 0..2 {
        let base = (header + k * record) as u16;
        for v in [
            base + 32,
            u16::MAX,
            base,
            u16::MAX,
            base + 96,
            message.len() as u16,
            u16::MAX,
        ] {
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    for _ in 0..2 {
        data.extend_from_slice(signer.pubkey().as_ref());
        data.extend_from_slice(sig.as_ref());
        data.extend_from_slice(message);
    }
    Instruction {
        program_id: ED25519_PROGRAM,
        accounts: vec![],
        data,
    }
}

/// L3 / mutation 5. The ticker inside the signed content hash is the STORED form: uppercased, digits kept. Hashing the
/// raw input or the confusable-mapped form would make every lowercase or digit-bearing ticker fail to verify.
#[test]
fn the_attested_ticker_is_the_stored_uppercase_form() {
    let mut env = attested_env();
    let w = env.wallet(20);

    let args = propose_args("b0b", 0, SOL);
    let att = attestation_ix(&env.canonicalizer, 0, &args, &w.pubkey());
    send_with(&mut env, &w, vec![att], &args).expect("signed over the stored form B0B");
    let stored = env.load::<Candidate>(&candidate_for(0, &args));
    assert_eq!(stored.ticker, "B0B");

    let other = propose_args("c1c", 0, SOL);
    assert_eq!(
        digest_with_ticker(&other, &w.pubkey(), "C1C"),
        attestation_message(0, &other, &w.pubkey()),
        "the test helper signs the stored form"
    );
    for wrong in ["c1c", "CIC"] {
        let bad = ed25519_ix(
            &env.canonicalizer,
            &digest_with_ticker(&other, &w.pubkey(), wrong),
        );
        assert_err!(
            send_with(&mut env, &w, vec![bad], &other),
            E::AttestationInvalid
        );
    }
    assert!(!env.exists(&candidate_for(0, &other)));
    let good = ed25519_ix(
        &env.canonicalizer,
        &digest_with_ticker(&other, &w.pubkey(), "C1C"),
    );
    send_with(&mut env, &w, vec![good], &other).expect("the stored form verifies");
}

/// Weak-test fix. The earlier two-signature case was refused inside the precompile and never reached the program.
/// Here both signatures are valid, so the refusal comes from the program's own count check.
#[test]
fn two_valid_signatures_are_refused_by_the_program_itself() {
    let mut env = attested_env();
    let w = env.wallet(10);
    let args = propose_args("BOB", 0, SOL);
    let message = attestation_message(0, &args, &w.pubkey());

    let two = two_signature_ix(&env.canonicalizer, &message);
    assert_err!(
        send_with(&mut env, &w, vec![two], &args),
        E::AttestationInvalid
    );
    assert!(!env.exists(&candidate_for(0, &args)));

    // the same signature as a normal one-signature instruction is accepted, so the refusal is about the count
    let one = ed25519_ix(&env.canonicalizer, &message);
    send_with(&mut env, &w, vec![one], &args).expect("exactly one signature");
}

/// An attestation is bound to the event: one signed for event 0 cannot be replayed on event 1 (L3, replay).
#[test]
fn an_attestation_for_one_event_does_not_work_on_another() {
    let mut env = attested_env();
    env.create_default_event("KXNBA-B");
    let w = env.wallet(10);
    let args = propose_args("BOB", 0, SOL);
    let for_event_zero = attestation_ix(&env.canonicalizer, 0, &args, &w.pubkey());
    let ix = propose(&w.pubkey(), 1, &env.treasury, &args);
    assert_err!(
        env.send(&[for_event_zero, ix], &[&w]),
        E::AttestationInvalid
    );
    assert!(!env.exists(&candidate_for(1, &args)));
}
