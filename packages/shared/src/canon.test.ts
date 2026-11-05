import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { renderGenerated } from "../scripts/gen-canon.js";
import { CANON_VERSION, CanonError, canonicalizeTicker, canonicalTickerHash, displayTicker, toHex, tryCanonicalizeTicker } from "./canon.js";

const read = (rel: string) => readFileSync(new URL(rel, import.meta.url), "utf8");

describe("ticker canon", () => {
  it("matches the shared fixtures (the Rust program checks the same file)", async () => {
    const lines = read("../canon/fixtures.tsv").split("\n").filter((l) => l !== "" && !l.startsWith("#"));
    expect(lines.length).toBeGreaterThanOrEqual(30);
    for (const line of lines) {
      const [input, expected, expectedHash] = line.split("\t") as [string, string, string];
      if (expected === "ERR") {
        expect(tryCanonicalizeTicker(input), `${JSON.stringify(input)} should be rejected`).toBeNull();
      } else {
        expect(canonicalizeTicker(input)).toBe(expected);
        expect(toHex(await canonicalTickerHash(input))).toBe(expectedHash);
      }
    }
  });

  it("the committed generated table is in sync with canon/confusables.txt", () => {
    expect(read("./canon-table.generated.ts")).toBe(renderGenerated(read("../canon/confusables.txt")));
  });

  it("is frozen at version 1", () => {
    expect(CANON_VERSION).toBe(1);
  });

  it("keeps digits for display but maps them for the canonical form", () => {
    expect(displayTicker("b0b")).toBe("B0B");
    expect(canonicalizeTicker("b0b")).toBe("BOB");
    expect(canonicalizeTicker("B0B")).toBe(canonicalizeTicker("bob"));
  });

  it("rejects non-ASCII before case mapping and reports why", () => {
    expect(() => displayTicker("ßß")).toThrow(CanonError);
    expect(() => displayTicker("A")).toThrow(/2 to 10/);
  });

  it("is idempotent", () => {
    for (const t of ["BOB", "b0b", "S5S", "1234567890"]) expect(canonicalizeTicker(canonicalizeTicker(t))).toBe(canonicalizeTicker(t));
  });
});
