//! Byte layout of every account (second review, mutation 10 and the "writes nothing reads back" list).
//!
//! The keeper and the indexer will filter accounts with `memcmp` at fixed offsets (docs/PHASE1_DESIGN.md section 2),
//! so a reordered field is a breaking change even though every Rust-level test would still pass. These offsets are
//! written out by hand and checked against the raw account bytes of a real scenario. Every field the instructions
//! write is read back here, so a dropped or misplaced write fails too.
mod common;

use anchor_lang::{prelude::Pubkey, AnchorSerialize, Space};
use common::ix::*;
use common::*;
use prequel::{
    Candidate, CoProposerShare, Commitment, Config, EventState, ExternalListing, Params, Vault,
};
use solana_signer::Signer;

fn raw(env: &Env, key: &Pubkey) -> Vec<u8> {
    env.svm.get_account(key).expect("account").data
}

fn bump_of(seeds: &[&[u8]]) -> u8 {
    Pubkey::find_program_address(seeds, &program_id()).1
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(d[at..at + 4].try_into().unwrap())
}
fn u64_at(d: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(d[at..at + 8].try_into().unwrap())
}
fn i64_at(d: &[u8], at: usize) -> i64 {
    i64::from_le_bytes(d[at..at + 8].try_into().unwrap())
}
fn zeros(d: &[u8], from: usize, to: usize, what: &str) {
    assert!(d[from..to].iter().all(|b| *b == 0), "{what} is zero");
}
fn bytes(d: &[u8], from: usize, expected: &[u8], what: &str) {
    assert_eq!(
        &d[from..from + expected.len()],
        expected,
        "{what} at {from}"
    );
}

#[test]
fn account_sizes_are_pinned() {
    // Growing a live account needs a migration, so these numbers change only on purpose.
    assert_eq!(8 + Config::INIT_SPACE, 422);
    assert_eq!(8 + EventState::INIT_SPACE, 527);
    assert_eq!(8 + ExternalListing::INIT_SPACE, 89);
    assert_eq!(8 + Candidate::INIT_SPACE, 627);
    assert_eq!(8 + Vault::INIT_SPACE, 57);
    assert_eq!(8 + Commitment::INIT_SPACE, 105);
    assert_eq!(8 + CoProposerShare::INIT_SPACE, 129);
    assert_eq!(Params::INIT_SPACE, 114);
}

#[test]
fn every_account_keeps_its_fields_at_the_documented_offsets() {
    let mut env = Env::new();
    env.init_config_with(|p| p.max_commit_per_wallet = 7 * SOL);
    let params = env.load::<Config>(&config_pda()).params;
    let mut params_bytes = Vec::new();
    params.serialize(&mut params_bytes).unwrap();
    assert_eq!(params_bytes.len(), 114);

    let (ev, ev_key) = env.create_default_event("KXLAYOUT-1");
    let (proposer, committer, co) = (env.wallet(50), env.wallet(50), env.wallet(50));
    let args = propose_args("b0b", 1, SOL / 10);
    let c = env.do_propose(&proposer, ev, &args).unwrap();
    env.warp_to(T0 + 10);
    env.do_commit(&committer, ev, &c, SOL / 5).unwrap();
    env.warp_to(T0 + 20);
    env.do_co_propose(&co, ev, &c, SOL / 4).unwrap();

    // ---- Config
    let d = raw(&env, &config_pda());
    assert_eq!(d.len(), 422);
    bytes(&d, 8, env.admin.pubkey().as_ref(), "admin");
    bytes(&d, 40, env.resolver.pubkey().as_ref(), "resolver");
    bytes(&d, 72, env.canonicalizer.pubkey().as_ref(), "canonicalizer");
    bytes(&d, 104, env.treasury.as_ref(), "treasury");
    zeros(&d, 136, 168, "pending_admin");
    bytes(&d, 168, &params_bytes, "params");
    assert_eq!(
        (d[282], d[283], d[284]),
        (0, 0, 1),
        "pause flags and version"
    );
    assert_eq!(d[285], bump_of(&[CONFIG_SEED_BYTES]), "bump");
    assert_eq!(u64_at(&d, 286), 1, "next_event_id");
    zeros(&d, 294, 422, "config reserved");

    // ---- EventState
    let d = raw(&env, &ev_key);
    assert_eq!(d.len(), 527);
    assert_eq!(u64_at(&d, 8), 0, "id");
    assert_eq!(&d[16..25], &[0, 0, 2, 1, 1, 255, 255, 0, bump_of(&[b"event", &0u64.to_le_bytes()])],
        "status Open, source, outcome_count, canon_version, layout_version, proposed, resolved, void_reason, bump");
    bytes(&d, 25, &sha256(b"KXLAYOUT-1"), "external_id_hash");
    assert_eq!(
        [
            i64_at(&d, 57),
            i64_at(&d, 65),
            i64_at(&d, 73),
            i64_at(&d, 81),
            i64_at(&d, 89)
        ],
        [T0, T0 + 3_600, T0 + 7_200, T0 + 7_200 + 10_800, T0],
        "window_open, window_close, event_start, expected_resolution, created_at"
    );
    zeros(
        &d,
        97,
        145,
        "resolution, dispute, resolved, launch and void timestamps",
    );
    zeros(&d, 145, 209, "evidence hash and disputer");
    assert_eq!(
        &d[209..217],
        &[0, 1, 0, 0, 0, 0, 0, 0],
        "candidate_count: one candidate on outcome 1"
    );
    bytes(&d, 217, &params_bytes, "frozen params");
    zeros(&d, 331, 459, "event reserved");
    assert_eq!(u32_at(&d, 459), 10, "external_id length");
    bytes(&d, 463, b"KXLAYOUT-1", "external_id");

    // ---- ExternalListing
    let listing = external_pda(&sha256(b"KXLAYOUT-1"));
    let d = raw(&env, &listing);
    assert_eq!(d.len(), 89);
    bytes(&d, 8, ev_key.as_ref(), "listing event");
    bytes(&d, 40, &sha256(b"KXLAYOUT-1"), "listing hash");
    assert_eq!(
        d[72],
        bump_of(&[b"ext", &sha256(b"KXLAYOUT-1")]),
        "listing bump"
    );
    zeros(&d, 73, 89, "listing reserved");

    // ---- Candidate
    let d = raw(&env, &c);
    assert_eq!(d.len(), 627);
    assert_eq!(
        &d[8..13],
        &[
            0,
            1,
            1,
            bump_of(&[b"candidate", ev_key.as_ref(), &[1], &args.canon_hash]),
            bump_of(&[b"vault", c.as_ref()])
        ],
        "status Pending, outcome, layout_version, bump, vault_bump"
    );
    bytes(&d, 13, ev_key.as_ref(), "event");
    bytes(&d, 45, proposer.pubkey().as_ref(), "proposer");
    bytes(&d, 77, &sha256(b"BOB"), "canon_hash (b0b becomes BOB)");
    bytes(&d, 109, &args.image_hash, "image_hash");
    assert_eq!(
        [u64_at(&d, 141), u64_at(&d, 149), u64_at(&d, 157)],
        [SOL / 10 + SOL / 5 + SOL / 4, 0, 0],
        "total_committed, refunded_total, claimed_commit_total"
    );
    assert_eq!(
        (u32_at(&d, 165), u32_at(&d, 169)),
        (3, 1),
        "committer_count, co_proposer_count"
    );
    assert_eq!(u64_at(&d, 173), SOL / 4, "co_proposer_total");
    assert_eq!(
        (i64_at(&d, 181), i64_at(&d, 189)),
        (T0, 0),
        "created_at, born_at"
    );
    zeros(&d, 197, 445, "launch results and reserved");
    assert_eq!(u32_at(&d, 445), 8);
    bytes(&d, 449, b"b0b Coin", "name");
    assert_eq!(u32_at(&d, 457), 3);
    bytes(&d, 461, b"B0B", "ticker, uppercased and unmapped");
    assert_eq!(u32_at(&d, 464), 14);
    bytes(&d, 468, b"ipfs://bafyb0b", "uri");

    // ---- Vault
    let d = raw(&env, &vault_pda(&c));
    assert_eq!(d.len(), 57);
    bytes(&d, 8, c.as_ref(), "vault candidate");
    assert_eq!(d[40], bump_of(&[b"vault", c.as_ref()]), "vault bump");
    zeros(&d, 41, 57, "vault reserved");

    // ---- Commitments: the proposer's (propose), the committer's (commit), the co-proposer's (co_propose)
    for (wallet, amount, at, has_share) in [
        (&proposer, SOL / 10, T0, 0u8),
        (&committer, SOL / 5, T0 + 10, 0),
        (&co, SOL / 4, T0 + 20, 1),
    ] {
        let d = raw(&env, &commitment_pda(&c, &wallet.pubkey()));
        assert_eq!(d.len(), 105);
        bytes(&d, 8, c.as_ref(), "commitment candidate");
        bytes(&d, 40, wallet.pubkey().as_ref(), "commitment wallet");
        assert_eq!(
            (u64_at(&d, 72), i64_at(&d, 80)),
            (amount, at),
            "amount, first_commit_ts"
        );
        assert_eq!(
            d[88],
            bump_of(&[b"commit", c.as_ref(), wallet.pubkey().as_ref()]),
            "commitment bump"
        );
        assert_eq!(d[89], has_share, "has_share");
        zeros(&d, 90, 105, "commitment reserved");
    }

    // ---- CoProposerShare
    let d = raw(&env, &coproposer_pda(&c, &co.pubkey()));
    assert_eq!(d.len(), 129);
    bytes(&d, 8, c.as_ref(), "share candidate");
    bytes(&d, 40, co.pubkey().as_ref(), "share wallet");
    assert_eq!(
        (u64_at(&d, 72), u64_at(&d, 80)),
        (SOL / 4, 0),
        "weight, claimed"
    );
    zeros(&d, 88, 104, "reward_debt");
    assert_eq!(i64_at(&d, 104), T0 + 20, "joined_at");
    assert_eq!(
        d[112],
        bump_of(&[b"coprop", c.as_ref(), co.pubkey().as_ref()]),
        "share bump"
    );
    zeros(&d, 113, 129, "share reserved");
}

const CONFIG_SEED_BYTES: &[u8] = b"config";
