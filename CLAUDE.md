# CLAUDE.md — Prequel

Prequel is a Solana launchpad for event-conditional memecoins. Traders commit SOL to unborn coins tied to future real-world events. A coin is minted only if its event happens, everyone who committed to a born coin pays the same price, and coins tied to the outcome that didn't happen refund 100%.

**SPEC.md is the source of truth.** This file is the short version you reload every session. If the two disagree, SPEC.md wins. If SPEC.md looks wrong or unclear, stop and raise it in `docs/DECISIONS.md` (section B, spec questions) and ask. Never silently change economics or security rules.

## Session protocol

1. Start: read CLAUDE.md, SPEC.md and PROGRESS.md. Check `docs/DECISIONS.md` for open spec questions that touch your work, and PROGRESS.md "Needs your decision" before any Phase 1 work.
2. Continue with the next unchecked item in the current phase. Plan first, build, then run all tests before marking anything done.
3. Work one phase at a time. Show a plan first, and wait for approval on anything expensive to reverse.
4. End: update PROGRESS.md (tick a box only when its criterion passes; add a session-log line) and `docs/DECISIONS.md` (one entry per decision). Commit and push to the session's working branch.

## Non-negotiables (SPEC §1)

- Localnet and devnet only. No mainnet deploys, no real funds, no mainnet keys anywhere in the repo.
- Users can always exit. Pausing may stop new proposals and commits, never claims or refunds.
- Nobody holds committers' SOL. It sits in a per-candidate program vault until refunded or locked as liquidity.
- No dev allocation, no presale, no platform token.
- Verify, don't recall. Before integrating any external program or API, read its current official docs and record the link and version in `docs/DECISIONS.md`. Do not write code against an API from memory. (Anchor is 1.2.2 and Agave 4.x; most remembered APIs are older.)
- Tests gate progress. A phase is done only when its "Done when" criteria in SPEC §11 pass.

## Working rules (SPEC §12)

- Start each session by reading CLAUDE.md, SPEC.md and PROGRESS.md. End it by updating PROGRESS.md and DECISIONS.md.
- Work one phase at a time. Show a plan first, and wait for approval on anything expensive to reverse.
- Read the current official docs before each integration (Anchor, Metaplex, the AMM, Kalshi, DFlow, storage), and record links and versions.
- Ask before adding any dependency outside the stack in SPEC §4.
- Make small commits with clear messages. Never commit secrets or keypairs.
- Never run anything against mainnet, never request real funds, and never write deploy scripts that target mainnet.
- If the spec is wrong or unclear, stop and ask. Don't change economics or security rules on your own.

## Stack (SPEC §4; exact pins in `docs/DECISIONS.md` D-001)

- **On-chain:** one Anchor 1.2.2 program on Agave 4.3.2. Metaplex Token Metadata via CPI. Classic SPL Token (D-006).
- **AMM:** Meteora DAMM v2 on its public 1% config 11, provisional and only with an atomic launch (D-002, Q-02). No in-house AMM.
- **Client SDK:** TypeScript generated from the IDL (generator pending, D-008).
- **Backend:** Node and TypeScript. Fastify API, indexer, keeper worker. Postgres via Drizzle; Redis with BullMQ.
- **Frontend:** Next.js App Router, TypeScript, Tailwind, shadcn/ui, TanStack Query, Solana wallet adapter (Phantom, Solflare, Backpack).
- **Media:** IPFS via Pinata, provisional (D-005), with Metaplex-standard JSON.
- **RPC:** Helius or similar, keys only in env.
- **Monorepo:** pnpm workspaces plus Turborepo.

## Repo layout (SPEC §4)

The repo root is the monorepo root (SPEC calls it `prequel/`; the GitHub repo is named `launch`).

```
programs/prequel/   Anchor program (Rust LiteSVM tests live in programs/prequel/tests)
packages/sdk/       generated client, PDA helpers, birth math
packages/shared/    constants, ticker canon lib, types, zod schemas
packages/db/        schema and migrations
apps/web/           Next.js app (users and admin)
apps/api/           REST and websocket API, media and Canon service
apps/indexer/       program logs to Postgres
apps/keeper/        Kalshi ingestion, resolver, cranks, reconciliation
tests/              cross-program and local-validator tests (empty so far)
scripts/            bootstrap, rust-checks, spikes/{kalshi,amm,media}
docs/               DECISIONS (ARCHITECTURE, THREAT_MODEL, RUNBOOK still to write)
SPEC.md  CLAUDE.md  PROGRESS.md
```

## Commands (verified 2026-11-05 unless marked planned)

Put the Solana tools on `PATH` first: `export PATH="$HOME/.local/share/solana/install/active_release/bin:$HOME/.cargo/bin:$PATH"`.

| Purpose | Command | Status |
| --- | --- | --- |
| Install pinned Agave, Anchor CLI, solana-verify (idempotent) | `scripts/bootstrap.sh` (`BOOTSTRAP_SKIP_VERIFY=1` skips solana-verify) | verified |
| Install JS deps | `pnpm install` | verified |
| JS lint, typecheck, test, web build (28 tasks) | `pnpm turbo run lint typecheck test build` | verified |
| Lint the scripts folder | `pnpm lint:scripts` | verified |
| Program: fmt, `anchor build`, clippy, LiteSVM tests | `scripts/rust-checks.sh` | verified |
| Build the program only | `anchor build` | verified |
| Kalshi spike (live, read-only) | `NODE_USE_ENV_PROXY=1 pnpm spike:kalshi` | verified |
| Media spike (dry run without `PINATA_JWT`) | `pnpm spike:media` | verified |
| AMM spike (devnet fixtures, build, LiteSVM tests) | `pnpm spike:amm` | verified |
| DB migrations | `pnpm --filter @prequel/db <migrate>` | planned |
| Web dev server | `pnpm --filter @prequel/web dev` | planned |
| End-to-end tests | `pnpm --filter @prequel/web exec playwright test` | planned |
| Verifiable build (before any public deploy) | `solana-verify build` (needs Docker) | planned |

## Security checklist for the program (SPEC §5, plus Phase 0 findings)

- Checked math with u128 intermediates; round user payouts down. AMM price and liquidity math needs 256-bit arithmetic.
- Constraints on every account: seeds, bump, `has_one`, owner, mint, token program. No unchecked remaining accounts. Reject duplicate mutable accounts.
- Pin CPI program ids (AMM, Metaplex, token program) as constants and check them. **A permissionless launch must build every AMM argument itself** (config, fee mode, price range, activation) and never take them from the caller (Q-12).
- **The coin's mint must not exist before the pool is created in the same transaction**, or a third party can create the pool first without holding the coin (verified, Q-02).
- Keep money totals in state (`total_committed` and similar). Never infer them from vault lamports: anyone can send lamports to a PDA.
- Payers for AMM rent must be data-less, system-owned PDAs (D-010).
- Every program-created PDA (mint, token accounts, vault) needs a creation path that tolerates pre-funding (`CreateAccountAllowPrefund` or top-up + allocate + assign); plain `create_account` fails if an attacker sends lamports first (verified).
- The position-owner PDA must never sign anything generic: after the lock `split_position` can still move the locked liquidity into another position (verified), and so could an NFT transfer. Sign only the exact fee-claim CPI.
- Close accounts safely and block revival.
- Time checks use the Clock sysvar through one effective-status function. Test the exact boundary seconds (T-1, T, T+1).
- Pause never blocks `claim`, `refund` or `claim_creator_fees`.
- Ed25519 attestation by instruction introspection: check the precompile program id, that signature, pubkey and message all come from the same instruction (all three index fields = u16::MAX), exactly one signature, the exact message bytes, and the expiry.
- Permissionless instructions are idempotent. Privileged instructions name their role (admin, resolver, canonicalizer).
- Admin, upgrade authority and treasury are a Squads multisig on devnet (Phase 10).
- Every invariant in SPEC §5 gets a test (with the rewording proposed in Q-05).

## Environment notes (cloud sandbox)

- The container is ephemeral. Commit and push before a session ends. A fresh container needs `scripts/bootstrap.sh` (about 5 minutes the first time; `anchor-cli` compiles from source).
- Network access is Full for this environment as of 2026-11-05. Still blocked for scripted clients: kalshi.com and solana.com (browser-verification checkpoint), and GitHub outside this repo (403). Docs sites that publish `llms.txt` can be read as Markdown (Kalshi, Meteora, Raydium, Pinata, Metaplex under `www.metaplex.com/docs`). Don't mock results for anything you cannot reach, and don't route around the policy.
- Node's built-in `fetch` ignores the proxy: export `NODE_USE_ENV_PROXY=1` for scripts that call the network. Node 22.23 runs `.ts` files directly (type stripping); keep spike scripts erasable-syntax only.
- Devnet reads (program and account dumps) are allowed and are how AMM fixtures are made; never read or write mainnet.
- Docker has no daemon here, so `solana-verify` and docker-compose cannot run; `psql` 16 and `redis-server` 7 are installed and can be started directly.
- Open a pull request only when the user asks. Work on the branch the session assigns.

## Decisions log

Every decision goes in `docs/DECISIONS.md` as one entry: options, choice, why, doc links and versions read (with the date), and an evidence tag ([V-me], [V-rev], [U]). Spec gaps go in as Q-entries and wait for the user's answer.

## Out of scope for v0 (SPEC §13)

Leave room in the design but don't build: multi-outcome ladders in the UI, trustless DFlow resolution, USDC commits, blinks, a Telegram bot, community-requested events, Kalshi Builder Codes and hedge-on-Kalshi, early-commit weighting, any platform token.
