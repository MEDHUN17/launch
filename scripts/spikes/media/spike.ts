// Spike D (Phase 0, task 0.7): media pipeline for a coin's image and Metaplex-style JSON.
//
//   node scripts/spikes/media/spike.ts                    dry run: builds and validates the JSON, no network
//   PINATA_JWT=... node scripts/spikes/media/spike.ts     also uploads image + JSON to Pinata (public IPFS)
//
// In this sandbox set NODE_USE_ENV_PROXY=1 so Node's fetch uses the proxy.
//
// No Pinata key was available when this was written, so the upload path is UNTESTED; it follows
// https://docs.pinata.cloud/files/uploading-files.md (POST https://uploads.pinata.cloud/v3/files, multipart,
// network=public, Bearer JWT; the response carries a CIDv1 such as bafybei...), read 2026-11-05.
// The on-chain field limits below (name 32, symbol 10, uri 200) are from memory of the Metaplex Token Metadata
// docs, which render client-side and could not be read here: verify them in Phase 3 before relying on them.

import { deflateSync, crc32 } from "node:zlib";

const LIMITS = { name: 32, symbol: 10, uri: 200 } as const; // TO VERIFY in Phase 3
const JWT = process.env.PINATA_JWT;

/** A valid 1x1 PNG, built by hand so the spike needs no binary fixture. */
function tinyPng(): Uint8Array {
  const chunk = (type: string, data: Uint8Array) => {
    const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body) >>> 0);
    return Buffer.concat([len, body, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(1, 0); // width
  ihdr.writeUInt32BE(1, 4); // height
  ihdr[8] = 8; // bit depth
  ihdr[9] = 2; // colour type: RGB
  const raw = Buffer.from([0, 255, 128, 0]); // filter byte + one RGB pixel
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

export function buildMetadata(args: { name: string; symbol: string; description: string; imageUri: string; eventTitle: string }) {
  const { name, symbol, description, imageUri, eventTitle } = args;
  if (name.length > LIMITS.name) throw new Error(`name > ${LIMITS.name}`);
  if (symbol.length > LIMITS.symbol) throw new Error(`symbol > ${LIMITS.symbol}`);
  return {
    name,
    symbol,
    description, // no yield or return language (SPEC §8)
    image: imageUri,
    attributes: [{ trait_type: "Event", value: eventTitle }],
    properties: { files: [{ uri: imageUri, type: "image/png" }], category: "image" },
  };
}

async function pinataUpload(bytes: Uint8Array | string, filename: string, mime: string) {
  const form = new FormData();
  form.append("file", new File([bytes], filename, { type: mime }));
  form.append("network", "public");
  const res = await fetch("https://uploads.pinata.cloud/v3/files", { method: "POST", headers: { Authorization: `Bearer ${JWT}` }, body: form });
  if (!res.ok) throw new Error(`pinata ${res.status}: ${await res.text()}`);
  const json = (await res.json()) as { data: { id: string; cid: string } };
  return json.data;
}

async function main() {
  const png = tinyPng();
  console.log(`image: ${png.length} bytes, PNG signature ok = ${Buffer.from(png.slice(0, 8)).toString("hex") === "89504e470d0a1a0a"}`);

  let imageUri = "ipfs://<image-cid>";
  if (JWT) {
    const up = await pinataUpload(png, "coin.png", "image/png");
    imageUri = `ipfs://${up.cid}`;
    console.log("uploaded image:", up);
  } else {
    console.log("PINATA_JWT not set: dry run (no network calls)");
  }

  const meta = buildMetadata({
    name: "Victory Coin",
    symbol: "VICT",
    description: "A coin that exists only if the event happens.",
    imageUri,
    eventTitle: "Example event",
  });
  const body = JSON.stringify(meta, null, 2);
  console.log(body);

  let metadataUri = "ipfs://<json-cid>";
  if (JWT) {
    const up = await pinataUpload(body, "metadata.json", "application/json");
    metadataUri = `ipfs://${up.cid}`;
    console.log("uploaded metadata:", up);
  }
  // A real CIDv1 string is 59 characters (bafybei...), so the on-chain URI is ~66 characters.
  const worstCase = "ipfs://" + "b".repeat(59);
  console.log(`uri length: ${metadataUri.length} (worst case ${worstCase.length}, limit ${LIMITS.uri}) ok=${worstCase.length <= LIMITS.uri}`);
}

if (process.argv[1] && import.meta.url.endsWith(process.argv[1].split("/").slice(-3).join("/"))) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
