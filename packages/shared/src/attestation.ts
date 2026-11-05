// Canonicalizer attestation message (decision D-014 version 1.1, SPEC §2 "Attestation"). Must agree byte for byte with
// programs/prequel/src/attestation.rs: canon/attestation-vectors.tsv holds known answers that both test suites check.

export const ATTESTATION_DOMAIN = "PREQUEL-PROPOSE-V1";
/** 18 (domain) + 3 x 32 (program, config, event) + 1 (outcome) + 4 x 32 (canon, content, image, proposer) + 8 (expiry). */
export const ATTESTATION_MESSAGE_LEN = 251;

type Bytes = Uint8Array<ArrayBuffer>;

export interface AttestationFields {
  /** 32-byte public keys. */
  programId: Uint8Array;
  config: Uint8Array;
  event: Uint8Array;
  outcome: number;
  /** sha256 of the canonical ticker (see `canonicalTickerHash`). */
  canonHash: Uint8Array;
  name: string;
  /** The stored, uppercased ticker (`displayTicker()`), not the canonical form and not the raw input. */
  displayTicker: string;
  uri: string;
  imageHash: Uint8Array;
  proposer: Uint8Array;
  /** Unix seconds. The program accepts the attestation while `now <= expiry`. */
  expiry: bigint;
}

const sha256 = async (bytes: Bytes): Promise<Bytes> => new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", bytes));

function concat(parts: Uint8Array[]): Bytes {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

function lengthPrefixed(text: string): Bytes {
  const bytes = new TextEncoder().encode(text);
  if (bytes.length > 0xffff) throw new RangeError("text is too long for a u16 length prefix");
  const prefix = new Uint8Array(2);
  new DataView(prefix.buffer).setUint16(0, bytes.length, true);
  return concat([prefix, bytes]);
}

function key32(name: string, bytes: Uint8Array): Uint8Array {
  if (bytes.length !== 32) throw new RangeError(`${name} must be 32 bytes, got ${bytes.length}`);
  return bytes;
}

/** sha256 of the name, the ticker and the URI, each preceded by its length in bytes as a u16 little-endian. */
export async function attestationContentHash(name: string, displayTicker: string, uri: string): Promise<Bytes> {
  return sha256(concat([lengthPrefixed(name), lengthPrefixed(displayTicker), lengthPrefixed(uri)]));
}

/** The 251 bytes that the program recomputes on-chain. */
export async function attestationMessage(f: AttestationFields): Promise<Bytes> {
  if (!Number.isInteger(f.outcome) || f.outcome < 0 || f.outcome > 255) throw new RangeError("outcome must be one byte");
  const expiry = new Uint8Array(8);
  new DataView(expiry.buffer).setBigInt64(0, f.expiry, true);
  const message = concat([
    new TextEncoder().encode(ATTESTATION_DOMAIN),
    key32("programId", f.programId),
    key32("config", f.config),
    key32("event", f.event),
    Uint8Array.of(f.outcome),
    key32("canonHash", f.canonHash),
    await attestationContentHash(f.name, f.displayTicker, f.uri),
    key32("imageHash", f.imageHash),
    key32("proposer", f.proposer),
    expiry,
  ]);
  if (message.length !== ATTESTATION_MESSAGE_LEN) throw new Error("attestation message has the wrong length");
  return message;
}

/** What the canonicalizer's Ed25519 key signs: the sha256 of the message. A digest keeps the signed transaction small. */
export async function attestationDigest(f: AttestationFields): Promise<Bytes> {
  return sha256(await attestationMessage(f));
}
