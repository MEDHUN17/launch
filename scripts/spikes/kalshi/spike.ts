// Spike B (Phase 0, task 0.5): Kalshi public market-data API.
//
//   node scripts/spikes/kalshi/spike.ts            (Node >= 22.18 strips the types; no dependencies)
//
// What it proves, against the live production *read-only* endpoints (no credentials, no funds):
//   1. list open, non-combo markets closing in the next 14 days (what the curator would pick from);
//   2. look up each market's series category (the v0 category allowlist/blocklist input);
//   3. read a settled market and map it to a Prequel outcome (YES = 0, NO = 1) or "needs a human";
//   4. hash the raw response (the evidence hash the resolver would submit) and check it is stable
//      across two fetches.
//
// Raw responses go to scripts/spikes/kalshi/.out/ (gitignored): Kalshi's data terms have not been
// reviewed yet (see docs/DECISIONS.md D-003), so no Kalshi payloads are committed.
//
// Docs read 2026-11-05: https://docs.kalshi.com/getting_started/api_environments.md,
// market_lifecycle.md, quick_start_market_data.md, api-reference/market/get-markets.md.

import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const BASE = process.env.KALSHI_BASE ?? "https://external-api.kalshi.com/trade-api/v2";
const OUT = join(dirname(fileURLToPath(import.meta.url)), ".out");
const UA = "prequel-phase0-spike";

type Json = Record<string, any>;
type Reply = { status: number; raw: Uint8Array; json: Json; url: string; date: string | null };

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const sha256 = (b: Uint8Array | string) => createHash("sha256").update(b).digest("hex");

/** GET with exponential backoff on 429/5xx (Kalshi documents 429s with no Retry-After header). */
async function get(path: string, query: Record<string, string | number> = {}): Promise<Reply> {
  const url = new URL(BASE + path);
  for (const [k, v] of Object.entries(query)) url.searchParams.set(k, String(v));
  for (let attempt = 0; attempt < 7; attempt++) {
    const res = await fetch(url, { headers: { "user-agent": UA, accept: "application/json" } });
    if (res.status === 429 || res.status >= 500) {
      await sleep(1000 * 2 ** attempt);
      continue;
    }
    const raw = new Uint8Array(await res.arrayBuffer());
    if (!res.ok) throw new Error(`${res.status} ${url}: ${new TextDecoder().decode(raw).slice(0, 200)}`);
    return { status: res.status, raw, json: JSON.parse(new TextDecoder().decode(raw)), url: url.toString(), date: res.headers.get("date") };
  }
  throw new Error(`gave up after retries: ${url}`);
}

/**
 * Map a Kalshi market to what the Prequel resolver should do.
 * Only `finalized` (terminal, positions paid out) is safe: `determined` can still be disputed/amended.
 * A final market whose result is not yes/no (scalar, or empty as for a void) is never auto-resolved.
 */
export type Action =
  | { kind: "wait"; reason: string }
  | { kind: "propose"; outcome: 0 | 1 }
  | { kind: "needs-human"; reason: string };

export function toAction(m: Json): Action {
  if (m.status !== "finalized") return { kind: "wait", reason: `status=${m.status}` };
  if (m.result === "yes") return { kind: "propose", outcome: 0 };
  if (m.result === "no") return { kind: "propose", outcome: 1 };
  return { kind: "needs-human", reason: `finalized with result=${JSON.stringify(m.result)}` };
}

/** Stable subset of a settled market: what we hash alongside the raw bytes. */
function canonicalEvidence(m: Json): string {
  const keys = ["ticker", "event_ticker", "status", "result", "settlement_ts", "settlement_timer_seconds", "close_time", "market_type"];
  return JSON.stringify(Object.fromEntries(keys.map((k) => [k, m[k] ?? null])));
}

async function main() {
  await mkdir(OUT, { recursive: true });
  console.log(`base: ${BASE}`);

  const status = await get("/exchange/status");
  console.log(`exchange_active=${status.json.exchange_active} trading_active=${status.json.trading_active}`);

  // 1. Curator view: non-combo markets closing within 14 days (status must be empty with close-ts filters).
  const now = Math.floor(Date.now() / 1000);
  const list = await get("/markets", { mve_filter: "exclude", min_close_ts: now, max_close_ts: now + 14 * 86400, limit: 200 });
  const open: Json[] = (list.json.markets as Json[]).filter((m) => m.status === "active");
  await writeFile(join(OUT, "markets-closing-14d.json"), list.raw);
  console.log(`markets returned=${list.json.markets.length} active=${open.length} next_cursor=${list.json.cursor ? "yes" : "no"}`);

  // 2. Category lives on the series (event.category is deprecated): one lookup per distinct event.
  const byEvent = new Map<string, Json>();
  for (const m of open) if (!byEvent.has(m.event_ticker)) byEvent.set(m.event_ticker, m);
  const categories = new Map<string, number>();
  let looked = 0;
  for (const [eventTicker, m] of byEvent) {
    if (looked++ >= 10) break; // stay polite; the keeper will cache series -> category
    const ev = await get(`/events/${eventTicker}`);
    const seriesTicker: string = ev.json.event.series_ticker;
    const series = await get(`/series/${seriesTicker}`);
    const cat: string = series.json.series.category;
    categories.set(cat, (categories.get(cat) ?? 0) + 1);
    console.log(`  ${m.ticker.padEnd(44)} close=${m.close_time} exp~${m.expected_expiration_time} cat=${cat} | ${String(m.title).slice(0, 60)}`);
  }
  console.log("categories seen:", Object.fromEntries(categories));

  // 3 + 4. A settled market -> action + evidence hash, fetched twice to check stability.
  const settledList = await get("/markets", { status: "settled", mve_filter: "exclude", limit: 5 });
  const pick: Json | undefined = (settledList.json.markets as Json[])[0];
  if (!pick) throw new Error("no settled market returned");
  const first = await get(`/markets/${pick.ticker}`);
  await sleep(1500);
  const second = await get(`/markets/${pick.ticker}`);
  const market: Json = first.json.market;
  await writeFile(join(OUT, `market-${pick.ticker}.json`), first.raw);
  const evidence = {
    url: first.url,
    fetched_date_header: first.date,
    status: market.status,
    result: market.result,
    settlement_ts: market.settlement_ts,
    settlement_timer_seconds: market.settlement_timer_seconds,
    raw_sha256: sha256(first.raw),
    raw_sha256_second_fetch: sha256(second.raw),
    raw_stable_across_fetches: sha256(first.raw) === sha256(second.raw),
    canonical_sha256: sha256(canonicalEvidence(market)),
    action: toAction(market),
  };
  await writeFile(join(OUT, `evidence-${pick.ticker}.json`), JSON.stringify(evidence, null, 2));
  console.log("settled market evidence:", JSON.stringify(evidence, null, 2));

  // Resolver mapping table (pure function; unit-tested in Phase 6).
  for (const sample of [
    { status: "finalized", result: "yes" },
    { status: "finalized", result: "no" },
    { status: "finalized", result: "scalar" },
    { status: "finalized", result: "" },
    { status: "determined", result: "yes" },
    { status: "disputed", result: "yes" },
  ]) console.log(`  toAction(${JSON.stringify(sample)}) =`, JSON.stringify(toAction(sample)));
}

// Run only when executed directly, so toAction can be imported by tests later.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
