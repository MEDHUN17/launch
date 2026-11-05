// Ticker canon (SPEC §2, decisions D-009 and D-015). Must agree with programs/prequel/src/canon.rs:
// the shared fixtures in canon/fixtures.tsv are checked by both test suites.
import { CANON_VERSION, CONFUSABLES } from "./canon-table.generated.js";

export { CANON_VERSION };
export const MIN_TICKER_LEN = 2;
export const MAX_TICKER_LEN = 10;

export class CanonError extends Error {
  constructor(
    readonly reason: "length" | "character",
    message: string,
  ) {
    super(message);
    this.name = "CanonError";
  }
}

/** Validates and returns the ticker uppercased (the form shown to users), without confusable mapping. */
export function displayTicker(input: string): string {
  // ASCII is checked first: toUpperCase() is Unicode-aware ("ß" becomes "SS", "ı" becomes "I").
  if (!/^[A-Za-z0-9]*$/.test(input)) throw new CanonError("character", "ticker may only contain ASCII letters and digits");
  if (input.length < MIN_TICKER_LEN || input.length > MAX_TICKER_LEN) {
    throw new CanonError("length", `ticker must be ${MIN_TICKER_LEN} to ${MAX_TICKER_LEN} characters`);
  }
  return input.toUpperCase();
}

/** Uppercase, then map confusables: the string that is hashed to form the candidate address seed. */
export function canonicalizeTicker(input: string): string {
  return [...displayTicker(input)].map((c) => CONFUSABLES[c] ?? c).join("");
}

export function tryCanonicalizeTicker(input: string): string | null {
  try {
    return canonicalizeTicker(input);
  } catch (e) {
    if (e instanceof CanonError) return null;
    throw e;
  }
}

/** sha256 of the canonical ticker: the `canon_hash` seed of the Candidate PDA (Web Crypto, so it also runs in browsers). */
export async function canonicalTickerHash(input: string): Promise<Uint8Array> {
  const bytes = new TextEncoder().encode(canonicalizeTicker(input));
  return new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", bytes));
}

export const toHex = (bytes: Uint8Array): string => [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
