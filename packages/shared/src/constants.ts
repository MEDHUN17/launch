// Constants fixed by SPEC §3. The program mirrors them in programs/prequel (Phase 1);
// a cross-language test will keep both sides equal.
export const TOKEN_DECIMALS = 6;
export const TOTAL_SUPPLY_TOKENS = 1_000_000_000n;
export const TOTAL_SUPPLY_BASE_UNITS = TOTAL_SUPPLY_TOKENS * 10n ** BigInt(TOKEN_DECIMALS);
export const MAX_OUTCOMES = 8;
export const OUTCOME_YES = 0;
export const OUTCOME_NO = 1;
export const BPS_DENOMINATOR = 10_000;
/** Harvested trading fees: proposer / co-proposers / treasury. Must sum to BPS_DENOMINATOR. */
export const FEE_SPLIT_BPS = { proposer: 3000, coProposers: 2000, treasury: 5000 } as const;
