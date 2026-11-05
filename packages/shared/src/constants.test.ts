import { describe, expect, it } from "vitest";
import { BPS_DENOMINATOR, FEE_SPLIT_BPS, TOTAL_SUPPLY_BASE_UNITS } from "./constants.js";

describe("SPEC §3 constants", () => {
  it("fee split sums to 10,000 bps", () => {
    const { proposer, coProposers, treasury } = FEE_SPLIT_BPS;
    expect(proposer + coProposers + treasury).toBe(BPS_DENOMINATOR);
  });
  it("supply is 1e9 tokens at 6 decimals", () => {
    expect(TOTAL_SUPPLY_BASE_UNITS).toBe(1_000_000_000_000_000n);
  });
});
