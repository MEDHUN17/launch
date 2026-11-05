# CLAUDE.md — Prequel

Prequel is a Solana launchpad for event-conditional memecoins. Traders commit SOL to unborn coins tied to future real-world events. A coin is minted only if its event happens, everyone who committed to a born coin pays the same price, and coins tied to the outcome that didn't happen refund 100%.

**SPEC.md is the source of truth.** This file is the short version you reload every session. If the two disagree, SPEC.md wins. If SPEC.md looks wrong or unclear, stop and raise it in `docs/DECISIONS.md` (section B, spec questions) and ask. Never silently change economics or security rules.

## Session protocol

1. Start: read CLAUDE.md, SPEC.md and PROGRESS.md. Check `docs/DECISIONS.md` for open spec questions that touch your work.
2. Continue with the next unchecked item in the current phase. Plan first, build, then run all tests before marking anything done.
3. Work one phase at a time. Show a plan first, and wait for approval on anything expensive to reverse.
4. End: update PROGRESS.md (tick a box only when its criterion passes; add a session-log line) and `docs/DECISIONS.md` (one entry per decision). Commit and push to the session's working branch.

## Non-negotiables (SPEC §1)

- Localnet and devnet only. No mainnet deploys, no real funds, no mainnet keys anywhere in the repo.
- Users can always exit. Pausing may stop new proposals and commits, never claims or refunds.
- Nobody holds committers' SOL. It sits in a per-candidate program vault until refunded or locked as liquidity.
- No dev allocation, no presale, no platform token.
- Verify, don't recall. Before integrating any external program or API, read its current official docs and record the link and version in `docs/DECISIONS.md`. Do not write code against an API from memory.
- Tests gate progress. A phase is done only when its "Done when" criteria in SPEC §11 pass.

## Working rules (SPEC §12)

- Start each session by reading CLAUDE.md, SPEC.md and PROGRESS.md. End it by updating PROGRESS.md and DECISIONS.md.
- Work one phase at a time. Show a plan first, and wait for approval on anything expensive to reverse.
- Read the current official docs before each integration (Anchor, Metaplex, the AMM, Kalshi, DFlow, storage), and record links and versions.
- Ask before adding any dependency outside the stack in SPEC §4.
- Make small commits with clear messages. Never commit secrets or keypairs.
- Never run anything against mainnet, never request real funds, and never write deploy scripts that target mainnet.
- If the spec is wrong or unclear, stop and ask. Don't change economics or security rules on your own.

## Stack (SPEC §4; versions are pinned in `docs/DECISIONS.md` D-001 once decided)

- **On-chain:** one Anchor program in Rust on the current Agave toolchain. Metaplex Token Metadata via CPI. Classic SPL Token unless Phase 0 finds a strong reason for Token-2022.
- **AMM:** Meteora DAMM v2 or Raydium CPMM, chosen in Phase 0 (D-002). No in-house AMM.
- **Client SDK:** TypeScript generated from the IDL with the generator current Anchor docs recommend (for example Codama).
- **Backend:** Node and TypeScript. Fastify API, indexer, keeper worker. Postgres via Drizzle; Redis with BullMQ.
- **Frontend:** Next.js App Router, TypeScript, Tailwind, shadcn/ui, TanStack Query, Solana wallet adapter (Phantom, Solflare, Backpack).
- **Media:** IPFS (Pinata) or Arweave, chosen in Phase 0 (D-005), with Metaplex-standard JSON.
- **RPC:** Helius or similar, keys only in env.
- **Monorepo:** pnpm workspaces plus Turborepo.

## Repo layout (SPEC §4)

The repo root is the monorepo root (SPEC calls it `prequel/`; the GitHub repo is named `launch`).

```
programs/prequel/   Anchor program
packages/sdk/       generated client, PDA helpers, birth math
packages/shared/    constants, ticker canon lib, types, zod schemas
packages/db/        schema and migrations
apps/web/           Next.js app (users and admin)
apps/api/           REST and websocket API, media and Canon service
apps/indexer/       program logs to Postgres
apps/keeper/        Kalshi ingestion, resolver, cranks, reconciliation
tests/              program integration tests
scripts/            spikes, devnet deploy, seed data
docs/               ARCHITECTURE, THREAT_MODEL, RUNBOOK, DECISIONS
SPEC.md  CLAUDE.md  PROGRESS.md
```

## Commands

**Status: PLANNED. Nothing below has been run yet.** Phase 0 task 0.2 replaces this table with verified commands. Do not trust a command until its status says `verified`. Package names and script names are tentative.

| Purpose | Command | Status |
| --- | --- | --- |
| Install JS deps | `pnpm install` | planned |
| Build, lint, typecheck, test all JS packages | `pnpm turbo run build lint typecheck test` | planned |
| Build the program | `anchor build` | planned |
| Rust lint | `cargo clippy --all-targets -- -D warnings` | planned |
| Rust unit tests | `cargo test` | planned |
| Program integration tests (LiteSVM, clock warping) | TBD in Phase 0/1 (D-007) | planned |
| Program tests on a local validator with cloned AMM and Metaplex | `anchor test` | planned |
| Regenerate the SDK from the IDL | TBD (D-008) | planned |
| DB migrations | `pnpm --filter @prequel/db <migrate>` | planned |
| Web dev server | `pnpm --filter web dev` | planned |
| End-to-end tests | `pnpm --filter web exec playwright test` | planned |
| Verifiable build (before any public deploy) | `solana-verify build` (needs Docker) | planned |

## Security checklist for the program (SPEC §5)

- Checked math with u128 intermediates; round user payouts down.
- Constraints on every account: seeds, bump, `has_one`, owner, mint, token program. No unchecked remaining accounts. Reject duplicate mutable accounts.
- Pin CPI program ids (AMM, Metaplex, token program) as constants and check them.
- Keep money totals in state (`total_committed` and similar). Never infer them from vault lamports: anyone can send lamports to a PDA.
- Close accounts safely and block revival.
- Time checks use the Clock sysvar. Test the exact boundary seconds (T-1, T, T+1).
- Pause never blocks `claim`, `refund` or `claim_creator_fees`.
- Ed25519 attestation by instruction introspection: check the precompile program id, that signature, pubkey and message all come from the same instruction, the exact message bytes, and the expiry.
- Permissionless instructions are idempotent. Privileged instructions name their role (admin, resolver, canonicalizer).
- Admin, upgrade authority and treasury are a Squads multisig on devnet (Phase 10).
- Every invariant in SPEC §5 gets a test.

## Environment notes (cloud sandbox)

- The container is ephemeral. Commit and push before a session ends.
- Not preinstalled: Solana/Agave CLI, Anchor CLI, `solana-verify`. Phase 0 task 0.1 adds an idempotent bootstrap script. Present: Rust, Node, pnpm, `psql` 16, `redis-server` 7. There is no Docker daemon, so use docker-compose in CI and on dev machines, and start Postgres and Redis directly here.
- Outbound network is allowlisted. Package registries (crates.io, npm, PyPI) work; other hosts may be denied. See "Blockers" in PROGRESS.md. If a host is denied, report it and carry on with work that doesn't need it. Don't mock or invent results for the missing part, and don't route around the policy.
- GitHub access is scoped to this repo. Attach other public repos only when the task needs them.
- Open a pull request only when the user asks. Work on the branch the session assigns.

## Decisions log

Every decision goes in `docs/DECISIONS.md` as one entry: options, choice, why, doc links and versions read (with the date). Spec gaps go in as Q-entries and wait for the user's answer.

## Out of scope for v0 (SPEC §13)

Leave room in the design but don't build: multi-outcome ladders in the UI, trustless DFlow resolution, USDC commits, blinks, a Telegram bot, community-requested events, Kalshi Builder Codes and hedge-on-Kalshi, early-commit weighting, any platform token.
