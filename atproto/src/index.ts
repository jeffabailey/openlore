// openlore opaque instance Worker (ADR-062 §1/§2).
//
// A DUMB front door: `GET /` serves a placeholder public card (the real card
// lands in 04-01); every other request is forwarded to the ONE Durable Object
// that owns this instance's records + manifest. The Worker computes no CID
// and never parses a record body.

import { OpenloreInstance } from "./instance";

export { OpenloreInstance };

export interface Env {
  readonly INSTANCE: DurableObjectNamespace<OpenloreInstance>;
}

/** One instance per deployment: every request reaches the same object. */
const INSTANCE_OBJECT_NAME = "openlore-instance";

const PLACEHOLDER_CARD =
  "<!doctype html><title>openlore instance</title><p>openlore opaque instance</p>";

function placeholderCard(): Response {
  return new Response(PLACEHOLDER_CARD, {
    status: 200,
    headers: { "content-type": "text/html; charset=utf-8" },
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const { pathname } = new URL(request.url);
    if (request.method === "GET" && pathname === "/") {
      return placeholderCard();
    }
    const instance = env.INSTANCE.get(env.INSTANCE.idFromName(INSTANCE_OBJECT_NAME));
    return instance.fetch(request);
  },
} satisfies ExportedHandler<Env>;
