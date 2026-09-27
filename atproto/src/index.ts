// openlore opaque instance Worker (ADR-062 §1/§2).
//
// A DUMB front door: every request — including `GET /`, the public read-only
// card (`card.ts`, rendered by the Durable Object from its committed manifest
// entries) — is forwarded to the ONE Durable Object that owns this instance's
// records + manifest. The Worker computes no CID and never parses a record
// body.
//
// Write auth (DV-4 / Q-SF-D2): every WRITE (any method other than GET/HEAD —
// in practice `PUT /records/:cid`) must carry `Authorization: Bearer <token>`
// matching the `OPENLORE_WRITE_TOKEN` Worker secret, compared in constant
// time; otherwise it is refused with 401 before reaching the Durable Object.
// Reads (`GET /`, `GET /manifest`, `GET /records/:cid`) stay public. If the
// secret is unset the Worker FAILS CLOSED: every write is refused.
//
// Setting the secret — it is NEVER in the repo, `wrangler.toml`, or CI:
//   deployed:  npx wrangler secret put OPENLORE_WRITE_TOKEN
//   local dev: OPENLORE_WRITE_TOKEN=<token> in the git-ignored `.dev.vars`
//              (the contract script generates a throwaway fixture there).

import { OpenloreInstance } from "./instance";

export { OpenloreInstance };

export interface Env {
  readonly INSTANCE: DurableObjectNamespace<OpenloreInstance>;
  /** The per-instance owner write token (a Worker secret); unset = no writes. */
  readonly OPENLORE_WRITE_TOKEN?: string;
}

/** One instance per deployment: every request reaches the same object. */
const INSTANCE_OBJECT_NAME = "openlore-instance";

// -----------------------------------------------------------------------------
// Write auth — pure decisions + one constant-time comparison.
// -----------------------------------------------------------------------------

/** Reads are public; every other method is a write and needs the owner token. */
export function isWrite(method: string): boolean {
  return method !== "GET" && method !== "HEAD";
}

/** The token presented as `Authorization: Bearer <token>`, or null. */
export function bearerTokenOf(authorization: string | null): string | null {
  const match = /^Bearer (\S+)$/.exec(authorization ?? "");
  return match?.[1] ?? null;
}

async function sha256(text: string): Promise<ArrayBuffer> {
  return crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
}

/**
 * Constant-time token check: both sides are hashed to fixed-length SHA-256
 * digests (so neither length nor content leaks through timing) and compared
 * with `crypto.subtle.timingSafeEqual`. An unset/empty expected token never
 * matches (fail closed).
 */
async function isOwnerToken(presented: string | null, expected: string | undefined): Promise<boolean> {
  if (presented === null || expected === undefined || expected === "") {
    return false;
  }
  const [presentedDigest, expectedDigest] = await Promise.all([sha256(presented), sha256(expected)]);
  return crypto.subtle.timingSafeEqual(presentedDigest, expectedDigest);
}

function unauthorizedWrite(): Response {
  return new Response("unauthorized write: missing or invalid owner token", {
    status: 401,
    headers: { "content-type": "text/plain", "www-authenticate": 'Bearer realm="openlore-instance"' },
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (
      isWrite(request.method) &&
      !(await isOwnerToken(bearerTokenOf(request.headers.get("authorization")), env.OPENLORE_WRITE_TOKEN))
    ) {
      return unauthorizedWrite();
    }
    const instance = env.INSTANCE.get(env.INSTANCE.idFromName(INSTANCE_OBJECT_NAME));
    return instance.fetch(request);
  },
} satisfies ExportedHandler<Env>;
