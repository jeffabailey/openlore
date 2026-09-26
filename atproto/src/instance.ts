// OpenloreInstance — the ONE Durable Object behind a user's openlore
// instance (ADR-062 §1/§2). It is an OPAQUE, content-addressed byte store:
//
//   PUT /records/:cid  store the body VERBATIM under :cid (first write wins;
//                      a re-PUT is an idempotent no-op on the blob). If the
//                      request carries `x-openlore-manifest-entry` (the CLI's
//                      ASCII-JSON display projection), that entry is APPENDED
//                      to the manifest — the manifest append IS the commit.
//   GET /records/:cid  the exact stored bytes, or 404.
//   GET /manifest      the manifest v1 envelope + committed entries, in
//                      append order.
//
// It never computes a CID and never parses a record body — the Rust
// `claim-domain` core is the sole canonicalizer. The only JSON it touches is
// the manifest-entry header, which it validates and then keeps VERBATIM.
// Behaviour mirrors `crates/test-support/src/fake_instance.rs` exactly.

import { DurableObject } from "cloudflare:workers";

export const MANIFEST_ENTRY_HEADER = "x-openlore-manifest-entry";

/** The manifest v1 discriminator envelope (Q-SF-D5 detection marker). */
const MANIFEST_ENVELOPE_PREFIX =
  '{"openlore":{"kind":"opaque-instance","contract_version":1},"records":[';
const MANIFEST_ENVELOPE_SUFFIX = "]}";

// -----------------------------------------------------------------------------
// Routes — a closed union; routing is a pure function of method + path.
// -----------------------------------------------------------------------------

export type InstanceRoute =
  | { readonly kind: "manifest" }
  | { readonly kind: "get-record"; readonly cid: string }
  | { readonly kind: "put-record"; readonly cid: string }
  | { readonly kind: "no-such-route" };

const RECORDS_PREFIX = "/records/";

export function routeOf(method: string, pathname: string): InstanceRoute {
  if (method === "GET" && pathname === "/manifest") {
    return { kind: "manifest" };
  }
  if (pathname.startsWith(RECORDS_PREFIX)) {
    const cid = pathname.slice(RECORDS_PREFIX.length);
    if (method === "GET") return { kind: "get-record", cid };
    if (method === "PUT") return { kind: "put-record", cid };
  }
  return { kind: "no-such-route" };
}

// -----------------------------------------------------------------------------
// Storage layout (DO transactional KV). Manifest entries are keyed by a
// zero-padded append sequence so `list()` returns them in append order.
// -----------------------------------------------------------------------------

const MANIFEST_LENGTH_KEY = "manifest-length";
const MANIFEST_ENTRY_PREFIX = "manifest-entry:";

const recordKey = (cid: string): string => `record:${cid}`;
const committedKey = (cid: string): string => `committed:${cid}`;
const manifestEntryKey = (sequence: number): string =>
  `${MANIFEST_ENTRY_PREFIX}${sequence.toString().padStart(16, "0")}`;

/** A manifest-entry header that parsed as JSON, kept byte-for-byte. */
type ManifestEntryHeader =
  | { readonly kind: "absent" }
  | { readonly kind: "valid"; readonly json: string }
  | { readonly kind: "malformed" };

function readManifestEntryHeader(headers: Headers): ManifestEntryHeader {
  const raw = headers.get(MANIFEST_ENTRY_HEADER);
  if (raw === null) return { kind: "absent" };
  try {
    JSON.parse(raw);
    return { kind: "valid", json: raw };
  } catch {
    return { kind: "malformed" };
  }
}

function manifestJson(committedEntries: Iterable<string>): string {
  return `${MANIFEST_ENVELOPE_PREFIX}${[...committedEntries].join(",")}${MANIFEST_ENVELOPE_SUFFIX}`;
}

function respond(status: number, contentType: string, body: BodyInit | null): Response {
  return new Response(body, { status, headers: { "content-type": contentType } });
}

// -----------------------------------------------------------------------------
// The Durable Object — the only place with effects (DO storage).
// -----------------------------------------------------------------------------

export class OpenloreInstance extends DurableObject {
  override async fetch(request: Request): Promise<Response> {
    const route = routeOf(request.method, new URL(request.url).pathname);
    switch (route.kind) {
      case "manifest":
        return this.serveManifest();
      case "get-record":
        return this.serveRecord(route.cid);
      case "put-record":
        return this.storeRecord(route.cid, request);
      case "no-such-route":
        return respond(404, "text/plain", "no such route");
    }
  }

  private async serveManifest(): Promise<Response> {
    const entries = await this.ctx.storage.list<string>({ prefix: MANIFEST_ENTRY_PREFIX });
    return respond(200, "application/json", manifestJson(entries.values()));
  }

  private async serveRecord(cid: string): Promise<Response> {
    const bytes = await this.ctx.storage.get<Uint8Array>(recordKey(cid));
    return bytes === undefined
      ? respond(404, "text/plain", "record not found")
      : respond(200, "application/octet-stream", bytes);
  }

  /**
   * Stage (and, with the header, commit) one record. All writes for one
   * request land in a single multi-key `put` — atomic in the DO — and the
   * DO's input gate serialises concurrent requests, so the append sequence
   * never races.
   */
  private async storeRecord(cid: string, request: Request): Promise<Response> {
    const body = new Uint8Array(await request.arrayBuffer());
    const entry = readManifestEntryHeader(request.headers);
    if (entry.kind === "malformed") {
      return respond(400, "text/plain", "malformed manifest entry header");
    }

    const storage = this.ctx.storage;
    const writes: Record<string, unknown> = {};

    // Content-addressed + idempotent: the first write wins.
    if ((await storage.get(recordKey(cid))) === undefined) {
      writes[recordKey(cid)] = body;
    }

    // The commit: append the display projection once per CID.
    if (entry.kind === "valid" && (await storage.get(committedKey(cid))) === undefined) {
      const length = (await storage.get<number>(MANIFEST_LENGTH_KEY)) ?? 0;
      writes[manifestEntryKey(length)] = entry.json;
      writes[committedKey(cid)] = true;
      writes[MANIFEST_LENGTH_KEY] = length + 1;
    }

    if (Object.keys(writes).length > 0) {
      await storage.put(writes);
    }
    return respond(201, "text/plain", null);
  }
}
