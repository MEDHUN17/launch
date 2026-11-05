import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { ATTESTATION_MESSAGE_LEN, attestationContentHash, attestationDigest, attestationMessage, type AttestationFields } from "./attestation.js";
import { toHex } from "./canon.js";

const fromHex = (hex: string): Uint8Array<ArrayBuffer> => Uint8Array.from(hex.match(/../g) ?? [], (h) => parseInt(h, 16));

type Row = [string, string, string, string, string, string, string, string, string, string, string, string, string, string, string];
const rows = readFileSync(new URL("../canon/attestation-vectors.tsv", import.meta.url), "utf8")
  .split("\n")
  .filter((l) => l !== "" && !l.startsWith("#"))
  .map((line) => line.split("\t") as Row);

function fieldsOf(row: Row): AttestationFields {
  const [, program, config, event, outcome, canonHash, name, displayTicker, uri, imageHash, proposer, expiry] = row;
  return {
    programId: fromHex(program),
    config: fromHex(config),
    event: fromHex(event),
    outcome: Number(outcome),
    canonHash: fromHex(canonHash),
    name,
    displayTicker,
    uri,
    imageHash: fromHex(imageHash),
    proposer: fromHex(proposer),
    expiry: BigInt(expiry),
  };
}

describe("attestation message", () => {
  it("matches the shared known answers (the Rust program checks the same file)", async () => {
    expect(rows.length).toBe(4);
    for (const row of rows) {
      const [name, , , , , , proposalName, displayTicker, uri, , , , contentHash, message, digest] = row;
      expect(toHex(await attestationContentHash(proposalName, displayTicker, uri)), `content hash of ${name}`).toBe(contentHash);
      const bytes = await attestationMessage(fieldsOf(row));
      expect(bytes.length).toBe(ATTESTATION_MESSAGE_LEN);
      expect(toHex(bytes), `message of ${name}`).toBe(message);
      expect(toHex(await attestationDigest(fieldsOf(row))), `digest of ${name}`).toBe(digest);
    }
  });

  it("every field changes the digest", async () => {
    const base = fieldsOf(rows[0]!);
    const flip = (bytes: Uint8Array) => Uint8Array.from(bytes, (b, i) => (i === 0 ? b ^ 1 : b));
    const changes: Partial<AttestationFields>[] = [
      { programId: flip(base.programId) },
      { config: flip(base.config) },
      { event: flip(base.event) },
      { outcome: base.outcome + 1 },
      { canonHash: flip(base.canonHash) },
      { name: `${base.name}!` },
      { displayTicker: `${base.displayTicker}X` },
      { uri: `${base.uri}x` },
      { imageHash: flip(base.imageHash) },
      { proposer: flip(base.proposer) },
      { expiry: base.expiry + 1n },
    ];
    const baseDigest = toHex(await attestationDigest(base));
    const seen = new Set([baseDigest]);
    for (const change of changes) {
      const digest = toHex(await attestationDigest({ ...base, ...change }));
      expect(seen.has(digest), `${Object.keys(change)[0]} must change the digest`).toBe(false);
      seen.add(digest);
    }
  });

  it("refuses malformed input instead of signing something else", async () => {
    const base = fieldsOf(rows[0]!);
    await expect(attestationMessage({ ...base, proposer: new Uint8Array(31) })).rejects.toThrow(RangeError);
    await expect(attestationMessage({ ...base, outcome: 256 })).rejects.toThrow(RangeError);
    await expect(attestationMessage({ ...base, outcome: -1 })).rejects.toThrow(RangeError);
    await expect(attestationMessage({ ...base, name: "x".repeat(70_000) })).rejects.toThrow(RangeError);
  });
});
