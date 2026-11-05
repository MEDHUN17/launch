//! Seeds, sizes and the hard caps of decision D-013 (see docs/PHASE1_DESIGN.md).

pub const CONFIG_SEED: &[u8] = b"config";
pub const EVENT_SEED: &[u8] = b"event";
pub const EXTERNAL_SEED: &[u8] = b"ext";
pub const CANDIDATE_SEED: &[u8] = b"candidate";
pub const VAULT_SEED: &[u8] = b"vault";
pub const COMMITMENT_SEED: &[u8] = b"commit";
pub const COPROPOSER_SEED: &[u8] = b"coprop";

pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
pub const MINUTE: i64 = 60;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;
pub const BPS_DENOMINATOR: u32 = 10_000;

pub const MIN_OUTCOMES: u8 = 2;
pub const MAX_OUTCOMES: u8 = 8;
/// `proposed_outcome` / `resolved_outcome` before resolution.
pub const NO_OUTCOME: u8 = u8::MAX;
pub const MAX_NAME_LEN: usize = 32;
pub const MAX_URI_LEN: usize = 200;
pub const MAX_EXTERNAL_ID_LEN: usize = 64;
/// Event source codes. v0 lists Kalshi markets only.
pub const SOURCE_KALSHI: u8 = 0;

// Hard caps on the economic settings (D-013). `update_config` and `initialize_config` reject anything outside.
pub const MAX_SUCCESS_FEE_BPS: u16 = 500;
pub const MIN_DISPUTE_WINDOW: i64 = MINUTE;
pub const MAX_DISPUTE_WINDOW: i64 = 7 * DAY;
pub const MIN_RESOLUTION_DELAY: i64 = DAY;
pub const MAX_RESOLUTION_DELAY: i64 = 30 * DAY;
pub const MIN_LAUNCH_WINDOW: i64 = 10 * MINUTE;
pub const MAX_LAUNCH_WINDOW: i64 = 7 * DAY;
pub const MIN_DISPUTE_BOND: u64 = LAMPORTS_PER_SOL / 100;
pub const MAX_DISPUTE_BOND: u64 = 10 * LAMPORTS_PER_SOL;
pub const MIN_PROPOSAL_FEE: u64 = LAMPORTS_PER_SOL / 1_000;
pub const MAX_PROPOSAL_FEE: u64 = LAMPORTS_PER_SOL;
pub const MIN_COMMIT_FLOOR: u64 = LAMPORTS_PER_SOL / 1_000;
pub const MAX_MIN_RAISE: u64 = 1_000 * LAMPORTS_PER_SOL;
pub const MIN_SLATE: u8 = 1;
pub const MAX_SLATE: u8 = 32;
/// `expected_resolution` may be at most this long after `event_start`.
pub const MAX_RESOLUTION_AFTER_START: i64 = 90 * DAY;

/// Domain tag of the canonicalizer's attestation message (D-014).
pub const ATTESTATION_DOMAIN: &[u8; 18] = b"PREQUEL-PROPOSE-V1";
