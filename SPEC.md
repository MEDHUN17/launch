# Prequel Build Prompt

Nov 5, 2026 · @MEDHUN

## How to use this

This doc is the full build prompt for Prequel. Export it, drop it into an empty repo, and let Claude Code build it one phase at a time.

1. Make an empty folder, run `git init`, and open Claude Code in it.
2. Export this doc as Markdown and save it in that folder as `SPEC.md`.
3. Paste the kickoff prompt below, then review the plan it shows you before it writes any code.
4. Start every later session with the next-session prompt.
5. Everything stays on localnet and devnet; mainnet is deliberately out of scope (see the last section).

Claude Code builds everything from the text below.

**Kickoff prompt**

```
Read SPEC.md in full. It is the build spec for Prequel, a Solana launchpad for event-conditional memecoins.

1. Create CLAUDE.md with the working rules (section 12), the stack and repo layout (section 4), and the commands you will use.
2. Create PROGRESS.md: every phase in section 11 with its "done when" criteria as checkboxes.
3. Create docs/DECISIONS.md: one entry per decision (options, choice, why, doc links and versions).
4. Start Phase 0. Show me your plan for it and wait for my go-ahead before writing code.

Work one phase at a time. End every session by updating PROGRESS.md and DECISIONS.md.
```

**Next-session prompt**

```
Read CLAUDE.md, SPEC.md and PROGRESS.md. Continue with the next unchecked item in the current phase. Plan first, build, then run all tests before marking anything done.
```

## 1. Mission and non-negotiables

You are building Prequel: a Solana launchpad where traders commit SOL to memecoins tied to future real-world events, and a coin is minted only if its event happens. Everyone who committed to a born coin pays the same price. Coins tied to the outcome that didn't happen refund 100%.

- **Localnet and devnet only.** No mainnet deploys, no real funds, no mainnet keys anywhere in the repo.
- **Users can always exit.** Pausing may stop new proposals and commits, but never claims or refunds.
- **Nobody holds committers' SOL.** It sits in a per-candidate program vault until it is refunded or locked as liquidity.
- **No dev allocation, no presale, no platform token.**
- **Verify, don't recall.** Before integrating any external program or API, read its current official docs and record the version in DECISIONS.md.
- **Tests gate progress.** A phase is done only when its criteria in section 11 pass.
- **The spec is the source of truth.** If something in it is wrong or unclear, stop and propose a change. Never silently change economics or security rules.

## 2. How Prequel works

Prequel turns an event calendar into a launchpad: each event carries a slate of unborn coins per outcome, and resolution decides which ones are born.

**Core objects**

- **Event:** a future real-world event with a resolution source. In v0 that is one binary Kalshi market, with outcomes YES = 0 and NO = 1. The data model supports up to 8 outcomes.
- **Slate:** the candidate coins proposed for one outcome of one event.
- **Candidate:** an unborn coin: name, ticker, image, metadata URI, its outcome, its proposer and its commit total.
- **Commitment:** one wallet's SOL committed to one candidate.
- **Born coin:** a winning-side candidate that met the minimum raise. It gets a mint, metadata, a locked liquidity pool and claimable allocations.

**Lifecycle**

1. **Listed.** A curator lists an event from the Kalshi feed and sets the commit window, which closes a set time before the event starts.
2. **Propose and commit.** Anyone proposes a candidate for an outcome (proposal fee plus an opening commit) or commits SOL to an existing one. Nothing is minted.
3. **Window closes.** A permissionless crank stops proposals and commits at the close time.
4. **Resolution.** After the event, the resolver proposes the outcome with an evidence hash, which opens a dispute window. Undisputed, anyone can finalize; disputed, the admin multisig decides or voids.
5. **Birth.** Each winning-side candidate with at least the minimum raise is born: fixed supply minted, success fee taken, a locked AMM pool seeded at the clearing price, claims opened.
6. **Refunds.** Losing-side candidates, winning-side candidates below the minimum raise, and every candidate of a voided event refund 100%, with no fees.
7. **Afterlife.** Born coins trade on the AMM. Pool trading fees are harvested and split between the proposer, co-proposers and the treasury.

**Canon dedup (feature)**

Prequel already clears every commit at one price at birth, so Canon adds the dedup rule and co-proposer credits.

- **One candidate per (event, outcome, canonical ticker).** The candidate PDA is seeded with the canonical ticker hash, so a duplicate can't be created even by calling the program directly.
- **Canonical ticker, computed on-chain.** Tickers are 2–10 characters of A–Z and 0–9. Uppercase them, map confusables (0→O, 1→I, 5→S), then hash. Keep the confusables table in one shared constant used by both the program and the TypeScript library.
- **Near-duplicate images.** The API computes a perceptual hash on upload. An image within the Hamming-distance threshold of one already in the slate is routed to co-propose instead.
- **Attestation (config flag).** When `require_attestation` is on, `propose_candidate` must carry an Ed25519 signature from the canonicalizer key over (event, outcome, canon hash, image hash, proposer, expiry), checked by instruction introspection.
- **Co-proposing.** Proposing an existing coin makes you a co-proposer, with at least the minimum commit. Co-proposers share their slice of trading fees pro-rata to committed SOL, so splitting across wallets gains nothing.

## 3. Economics and parameters

Every born coin's pool opens at the same price committers paid, and all committed SOL except the fee and pool costs is locked as liquidity forever.

**Supply and authorities**

- 1,000,000,000 tokens, 6 decimals, minted once at birth.
- Mint authority revoked right after minting. No freeze authority. Metadata immutable.
- Classic SPL Token plus Metaplex metadata, unless Phase 0 finds a strong reason for Token-2022.

**Birth math** (u128, round user payouts down)

```latex
\begin{aligned}
\text{fee} &= \text{total} \times \text{fee\_bps} / 10000 \\
\text{pool\_sol} &= \text{total} - \text{fee} - \text{pool\_costs} \\
\text{lp\_tokens} &= \text{supply} \times \frac{\text{pool\_sol}}{\text{total} + \text{pool\_sol}} \\
\text{committer\_tokens} &= \text{supply} - \text{lp\_tokens} \\
\text{allocation}_i &= \left\lfloor \frac{\text{commit}_i \times \text{committer\_tokens}}{\text{total}} \right\rfloor
\end{aligned}
```

- This split makes the pool price (pool\_sol ÷ lp\_tokens) equal the clearing price (total ÷ committer\_tokens).
- `pool_costs` = rent and any creation fee the AMM charges, paid from the vault.
- With a 1% fee and small pool costs, about 502.5M tokens go to committers and about 497.5M to the pool.
- Rounding dust stays in the token vault. Decide in Phase 3 whether `sweep_dust` burns it or adds it to the pool, and log the choice.

**Fees**

- Proposal fee: non-refundable, to the treasury (anti-spam).
- Success fee: 1% of a born candidate's total, to the treasury.
- Refunds: 0%.
- Pool trading fee: 1%, or the closest tier the AMM supports. Collect it in SOL only if the AMM allows; otherwise the ledger tracks both assets.
- Harvested trading fees: 30% proposer, 20% co-proposers, 50% treasury.

**Defaults** (all stored in Config; devnet can use shorter times)

| Parameter | Default | Notes |
| --- | --- | --- |
| min\_commit | 0.05 SOL | per commit |
| max\_commit\_per\_wallet | 50 SOL | per candidate; 0 means no cap |
| min\_raise | 5 SOL | a winning candidate below this refunds |
| max\_candidates\_per\_outcome | 12 | keeps slates readable |
| window close | 60 min before event start | set per event |
| dispute\_window | 2 h | 5 min on devnet |
| dispute\_bond | 1 SOL | returned if the dispute is upheld |
| max\_resolution\_delay | 7 days | after this, anyone can void |
| proposal\_fee | 0.02 SOL | to the treasury |
| success\_fee\_bps | 100 | 1% |
| fee split (proposer / co-proposers / treasury) | 3000 / 2000 / 5000 bps | must sum to 10,000 |

## 4. Architecture, stack and repo layout

One Anchor program holds all funds and rules. Off-chain services only list events, index, crank and resolve, and a TypeScript monorepo holds everything else.

**Stack** (latest stable versions, pinned and logged in DECISIONS.md)

- **On-chain:** one Anchor program in Rust on the current Agave toolchain.
- **Token metadata:** Metaplex Token Metadata via CPI.
- **AMM:** chosen in Phase 0 between Meteora DAMM v2 and Raydium CPMM. Criteria: pool creation by CPI, a permanent LP lock that can still claim fees, a 1% fee tier, Jupiter routing. No in-house AMM.
- **Client SDK:** TypeScript, generated from the IDL with the generator current Anchor docs recommend (for example Codama).
- **Backend:** Node and TypeScript: a Fastify API, an indexer and a keeper worker. Postgres via Drizzle; Redis with BullMQ for jobs.
- **Frontend:** Next.js App Router, TypeScript, Tailwind, shadcn/ui, TanStack Query, Solana wallet adapter (Phantom, Solflare, Backpack).
- **Media:** IPFS (Pinata) or Arweave, chosen in Phase 0, with Metaplex-standard JSON.
- **RPC:** Helius or similar, keys only in env.
- **Monorepo:** pnpm workspaces plus Turborepo.

**Repo layout**

```
prequel/
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

## 5. On-chain program

The program enforces every money rule. Anything permissionless must be idempotent, and anything privileged needs a named role.

**Accounts (PDAs)**

| Account | Seeds | Holds |
| --- | --- | --- |
| Config | `config` | admin (multisig), resolver, canonicalizer, treasury, all section 3 parameters, `require_attestation`, pause flags, version, next event id |
| EventState | `event`, event id | source and external market id, outcome count, window open and close, event start, expected resolution, status, proposed and resolved outcome, evidence hash, dispute deadline, disputer, candidate count per outcome |
| ExternalListing | `ext`, hash of external id | stops the same Kalshi market being listed twice |
| Candidate | `candidate`, event, outcome, canon hash | name, ticker, metadata URI, image hash, proposer, total committed, committer count, co-proposer total, status, mint, pool, allocation numbers |
| Vault | `vault`, candidate | this candidate's SOL only, never mixed with another's |
| Commitment | `commit`, candidate, wallet | amount, first commit time, claimed or refunded; closed on claim or refund |
| CoProposerShare | `coprop`, candidate, wallet | committed SOL and fees claimed; survives token claims |
| Mint | `mint`, candidate | the born coin; its token vault is an ATA owned by the candidate |
| FeeLedger | `fees`, candidate | harvested totals, proposer claimable, co-proposer reward-per-lamport accumulator |
| DisputeBond | `bond`, event | the disputer's bond |

Name the real-world event account `EventState` so it never clashes with Anchor's `#[event]` logs.

**Instructions**

| Instruction | Caller | Does |
| --- | --- | --- |
| initialize\_config | deployer | one-time setup |
| update\_config | admin | change parameters and keys |
| set\_pause | admin | pause proposals and/or commits only |
| create\_event | admin | list an event, validate its times, create ExternalListing |
| propose\_candidate | anyone | compute the canon hash on-chain, check the slate cap, verify attestation if required, take proposal fee and opening commit |
| co\_propose | anyone | join an existing candidate with at least min\_commit; create CoProposerShare |
| commit | anyone | add SOL before window close; enforce caps |
| close\_window | anyone | after the close time |
| propose\_resolution | resolver | outcome plus evidence hash, only after event start; opens the dispute window |
| dispute\_resolution | anyone | post the bond inside the dispute window |
| settle\_dispute | admin | set the final outcome or void; return or forfeit the bond |
| finalize\_resolution | anyone | after an undisputed dispute window |
| void\_event | admin, or anyone after max\_resolution\_delay | every candidate refunds |
| birth\_candidate | anyone | winning side with min raise met: mint supply, write metadata, revoke mint authority, pay the fee, set allocations |
| seed\_pool | anyone | wrap pool SOL, create the AMM pool by CPI at the clearing price, lock the LP permanently |
| claim | committer | receive the allocation; close Commitment |
| refund | committer | receive 100% of the commit; close Commitment |
| harvest\_fees | anyone | claim fees on the locked LP by CPI; split them into FeeLedger |
| claim\_creator\_fees | proposer or co-proposer | withdraw the accrued share |
| sweep\_dust | anyone, after a grace period | handle rounding dust as DECISIONS.md says |

**Status machines** (test every allowed transition and reject every other one)

- EventState: Open → Closed → ResolutionProposed → Resolved. ResolutionProposed → Disputed → Resolved or Voided.
- Voiding: the admin can void an Open, Closed or Disputed event. Anyone can void a Closed or Disputed event after max\_resolution\_delay, so funds never get stuck.
- Candidate: Pending → Born, or Pending → Refunding.

**Logs** (`emit!` one per state change, for the indexer)

EventCreated, CandidateProposed, CoProposed, Committed, WindowClosed, ResolutionProposed, ResolutionDisputed, DisputeSettled, ResolutionFinalized, EventVoided, CandidateBorn, PoolSeeded, Claimed, Refunded, FeesHarvested, CreatorFeesClaimed, ConfigUpdated, PauseChanged.

**Invariants** (each one gets a test)

1. The sum of Commitment amounts equals the candidate's total committed.
2. A refunding candidate returns exactly each commit, and its vault ends at the rent-exempt minimum.
3. For a born candidate, fee + pool costs + pool SOL = total, committer tokens + LP tokens = supply, and no mint authority remains.
4. A Commitment is claimed or refunded, never both, and at most once.
5. No proposal or commit after close, no birth before finalization, no birth on the losing side; a voided event only refunds.
6. Pause never blocks claim, refund or claim\_creator\_fees.
7. Every committer gets the same tokens per lamport, up to floor rounding.
8. At most one candidate exists per (event, outcome, canon hash).
9. Only the resolver proposes outcomes and only the admin settles disputes; cranks are permissionless and idempotent.

**Security rules**

- Checked math with u128 intermediates; round user payouts down.
- Anchor constraints on every account: seeds, bump, has\_one, owner, mint, token program. No unchecked remaining accounts.
- Pin CPI program ids (AMM, Metaplex, token program) as constants and check them.
- Close accounts safely and block revival. Reject duplicate mutable accounts.
- Time checks use the Clock sysvar; test the exact boundary seconds.
- Admin, upgrade authority and treasury are a Squads multisig, on devnet too.
- Use address lookup tables if `seed_pool` exceeds transaction account limits.
- Produce a verifiable build (solana-verify) before any public deploy.

## 6. Off-chain services

Off-chain code never holds user funds: it lists events, watches resolution sources, cranks permissionless instructions and serves data.

**Event ingestion (apps/keeper)**

- Pull open markets from Kalshi's official API. Read its docs and API terms first.
- Cache them in `external_markets`; a curator picks which to list.
- The curator sets the real event start time, because a market's close time isn't always the start.
- v0 categories: sports, culture and entertainment, tech launches.
- Blocked: elections and politics, deaths, disasters, violence, private individuals, and anything a participant can influence.

**Resolver (apps/keeper)**

- Watch listed events. When the Kalshi market settles, save the API response, hash it (sha256) and submit `propose_resolution` with that evidence hash.
- If Kalshi voids or cancels the market, or the event slips past max\_resolution\_delay, prepare a void for the admin.
- Alert on anything unexpected. Never settle disputes automatically.
- Research only, no code: can the program read the settlement of DFlow's tokenized Kalshi markets on-chain? Start from [DFlow's Prediction Markets API announcement](https://solana.com/news/dflow-prediction-markets-api). If yes, draft trustless resolution for v1 in DECISIONS.md.

**Cranks (apps/keeper)**

- Jobs: close\_window, finalize\_resolution, birth\_candidate, seed\_pool, sweep\_dust, and harvest\_fees on an interval.
- Every job is idempotent, simulated before sending, retried with backoff and sent with priority fees.
- Reconciliation every few minutes compares on-chain state with Postgres and alerts on any mismatch.

**Indexer (apps/indexer)**

- Subscribe to program logs (Helius webhooks or websocket), decode Anchor logs and upsert into Postgres.
- Backfill from signatures on restart. Use finalized commitment for money tables.
- Tables: events, candidates, commitments, coproposer\_shares, resolutions, disputes, born\_coins, pools, fee\_harvests, external\_markets, evidence\_snapshots, moderation\_flags, jobs.

**API (apps/api)**

- REST plus websocket: calendar and event list, event detail with slates and live totals, candidate detail.
- Wallet portfolio: pending commits, claimable tokens, refunds and creator fees.
- Canon check, media upload, attestation signing and share-card images.
- Admin endpoints behind Sign-In With Solana and an admin wallet allowlist.
- zod validation and rate limits on every endpoint.

**Media and Canon service (apps/api)**

- Image upload: check type and size, strip EXIF, compute a perceptual hash, check the slate for near-duplicates.
- Store the image and Metaplex JSON (IPFS or Arweave, per Phase 0) and return the URI.
- The canonicalizer key signs short-lived attestations. It lives in env, separate from every other key.
- Ticker canon code lives in packages/shared, with tests proving the TypeScript and Rust hashes match.

## 7. Frontend and admin console

The app is a calendar of events, where each event page shows the coins waiting to be born on each side.

**User pages**

- **Home:** upcoming events with countdowns to window close, category filters and the busiest slates.
- **Event:** the question and a link to its resolution source, with YES coins and NO coins side by side (victory coin vs cope coin). Each card shows image, ticker, total committed, committers, implied clearing price and your commit.
- **Propose:** name, ticker and image, with a live Canon check: "$XYZ already exists — co-propose instead."
- **Hedge both:** one commit split across a YES coin and a NO coin.
- **Candidate:** details, commit history, co-proposers and a share card.
- **Portfolio:** pending commits, claimable tokens, refunds and creator fees, each one tap away.
- **Born coin:** pool link, price from a DexScreener or Birdeye embed, and a Jupiter swap link.

**Admin console**

- List events from the Kalshi feed and set their windows and start times.
- Hide candidates from the UI; they stay on-chain.
- Resolution panel showing the evidence, a dispute queue, the pause switch and a read-only config view.

**UX rules**

- Every commit screen states "Full refund if \[other outcome\] happens."
- Explain uniform pricing in one line next to the commit button.
- Simulate every transaction first and map program errors to plain messages.
- Mobile first. Works with Phantom, Solflare, Backpack and mobile wallets.
- Share cards (OG images) for each candidate and each born coin, with X post links.

## 8. Compliance and safety features

These ship in v0, so a lawyer can review the product as it would actually run.

- Geoblocking at the edge from a country list in config, with a blocked page.
- 18+ confirmation and acceptance of terms and a risk disclosure before a wallet's first commit.
- The event category blocklist from section 6, enforced in the admin console and the keeper.
- Ticker and name filter for slurs, real people's names and brand impersonation; admins can hide candidates.
- No yield or return language anywhere in UI copy or token metadata.
- Optional sanctions screening of wallets before commit.

## 9. Testing

Program tests come first, and every invariant in section 5 has a passing test before its phase is marked done.

- Rust unit tests for the birth math and ticker canon.
- Program integration tests on LiteSVM (or the current recommended fast framework), with clock warping.
- `anchor test` on a local validator with the chosen AMM and Metaplex programs cloned in.
- Property tests for the invariants, and fuzzing with Trident.
- Cross-language tests: the TypeScript and Rust canon hashes match on a shared fixture list.
- API and indexer tests against a local validator plus Postgres in docker-compose.
- Playwright end to end: propose → commit → resolve → birth → claim, plus every refund path.
- Load test: 1,000 commits across 50 candidates, then every claim and refund succeeds.

## 10. DevOps

There are two environments only: localnet for development and devnet for the beta.

- **CI (GitHub Actions):** clippy, eslint, typecheck, unit and program tests, and the web build on every push.
- **Devnet deploy:** program and IDL via Anchor; web on Vercel; api, indexer and keeper on Railway or Fly.io; Postgres on Neon or Supabase.
- **Secrets:** env only, with `.env.example` committed. Keeper, resolver and canonicalizer keys are separate from admin and never in git.
- **Monitoring:** Sentry, structured logs, and alerts for keeper failures, reconciliation mismatches and resolver anomalies.
- **Docs to keep current:** docs/ARCHITECTURE.md, docs/THREAT\_MODEL.md and docs/RUNBOOK.md (dispute, void, pause, key rotation).

## 11. Build phases

Build in this order. A phase is done only when everything in its "Done when" column passes.

| Phase | Build | Done when |
| --- | --- | --- |
| 0. Setup and spikes | Monorepo, pinned toolchain, CLAUDE.md, CI skeleton. Spikes: AMM choice (create a pool, lock the LP and claim fees by CPI on localnet); Kalshi API (list markets, read a settled one); DFlow on-chain settlement (research only); media storage | Spike scripts run; four decisions logged |
| 1. Program core | Config, events, propose with Canon, co-propose, commit, close, void, refund | Happy and failure tests for every instruction; invariants 1, 2, 4, 5 and 8 pass |
| 2. Resolution | Propose, dispute, settle, finalize, delay void, evidence hash | Clock-warp tests cover every status path |
| 3. Birth and claims | Mint, metadata, fee, allocations, claim, dust rule | Invariants 3 and 7 pass |
| 4. Pool and fees | seed\_pool by CPI, LP lock, harvest, creator fee claims | Works on localnet with the cloned AMM; fee split exact to the lamport |
| 5. SDK, indexer, DB, API | Generated client, schema, log decoding, endpoints | API shows the correct live state for a scripted event |
| 6. Keeper and resolver | Ingestion, cranks, resolution, reconciliation, alerts | A scripted event runs end to end, unattended, on localnet |
| 7. Frontend | Every page in section 7 | Playwright happy and refund paths pass |
| 8. Admin and compliance | Console, geoblock, terms, filters | Admin can list, hide, resolve and pause |
| 9. Hardening | Fuzzing, self-review against the section 5 rules, load test, threat model | No open high-severity findings |
| 10. Devnet beta | Deploy everything; run three events on real schedules | Full lifecycle seen on devnet; RUNBOOK.md complete; mainnet checklist drafted, not run |

## 12. Working rules for Claude Code

These rules apply in every session; copy them into CLAUDE.md.

- Start each session by reading CLAUDE.md, SPEC.md and PROGRESS.md. End it by updating PROGRESS.md and DECISIONS.md.
- Work one phase at a time. Show a plan first, and wait for approval on anything expensive to reverse.
- Read the current official docs before each integration (Anchor, Metaplex, the AMM, Kalshi, DFlow, storage), and record links and versions.
- Ask before adding any dependency outside the stack in section 4.
- Make small commits with clear messages. Never commit secrets or keypairs.
- Never run anything against mainnet, never request real funds, and never write deploy scripts that target mainnet.
- If the spec is wrong or unclear, stop and ask. Don't change economics or security rules on your own.

## 13. Out of scope for v0

Leave room for these in the design, but don't build them yet.

- Multi-outcome ladders in the UI (the data model already supports 8 outcomes).
- Trustless resolution from DFlow settlement, if Phase 0 finds it possible.
- USDC commits.
- Committing straight from X posts via Solana Actions (blinks).
- A Telegram alerts bot.
- Community-requested events.
- Kalshi Builder Codes and a "hedge on Kalshi" button through DFlow.
- Extra weight for early commits.
- Any platform token.

## Before mainnet (for you, not Claude Code)

Real money waits until every item here is done.

- An independent security audit, then a bug bounty.
- Legal advice for where you operate and where your users are: event-contract rules (such as the US CFTC's), gambling law and securities law.
- In India, the Promotion and Regulation of Online Gaming Act, 2025 bans all online real-money games and has been in force since 1 May 2026 ([Supreme Court Observer](https://www.scobserver.in/cases/challenge-to-the-promotion-and-regulation-of-online-gaming-act-2025/)). A Supreme Court challenge was still pending as of July 2026. Ask a lawyer whether event-linked commits count.
- A Squads multisig for admin, upgrade authority and treasury, with keys on hardware wallets.
- A first launch with curated low-risk events and low raise caps.
- Rehearsed runbooks: dispute, void, pause, key rotation.
