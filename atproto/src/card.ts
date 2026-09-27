// The public read-only philosophy card (ADR-062 §1, US-SF-005, D-7).
//
// `GET /` renders the committed manifest entries — the display projection
// the REAL `openlore` CLI wrote on push — as a read-only HTML page. Pure
// functions only: the Durable Object hands in the stored entry strings and
// serves the result. The card computes no CID and never reads a record body.
//
// THE CARD CONTRACT (shared with `crates/test-support/src/fake_instance.rs`
// and asserted by `scripts/contract-roundtrip.sh` + the Rust acceptance suite):
//
// * one `<li class="claim" data-cid="…" data-author="…">` row per committed
//   entry, in manifest order, attributed to that entry's ONE `author_did` —
//   there is no merged/consensus row shape (anti-merging, ADR-016);
// * an empty manifest renders `<p class="empty-state">no claims published
//   yet</p>` — a valid 200 page, not an error;
// * no authoring/edit control and no script; every field HTML-escaped;
// * served as `text/html; charset=utf-8` with a restrictive CSP.

/** The card needs no script, no fetch, no form, no framing. */
export const CARD_CONTENT_SECURITY_POLICY =
  "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

const HTML_ESCAPES: Readonly<Record<string, string>> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
};

/** Escape text for HTML text AND attribute context (`& < > " '`). */
export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (c) => HTML_ESCAPES[c] ?? c);
}

/**
 * A manifest entry field as display text: strings verbatim, finite numbers
 * in their shortest form, anything else (absent / non-scalar) empty.
 */
function displayField(entry: unknown, name: string): string {
  if (typeof entry !== "object" || entry === null) return "";
  const value: unknown = (entry as Record<string, unknown>)[name];
  if (typeof value === "string") return value;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  return "";
}

/** Parse one stored entry string; an unparseable one renders as empty fields. */
function parseEntry(entryJson: string): unknown {
  try {
    return JSON.parse(entryJson) as unknown;
  } catch {
    return null;
  }
}

/** One card row: the claim, attributed to its ONE author. */
function renderRow(entry: unknown): string {
  const field = (name: string): string => escapeHtml(displayField(entry, name));
  const cid = field("cid");
  const author = field("author_did");
  return (
    `<li class="claim" data-cid="${cid}" data-author="${author}">` +
    `<span class="subject">${field("subject")}</span> ` +
    `<span class="predicate">${field("predicate")}</span> ` +
    `<span class="object">${field("object")}</span> ` +
    `<span class="confidence">confidence ${field("confidence")}</span> ` +
    `<span class="author">by ${author}</span> ` +
    `<span class="cid">${cid}</span></li>`
  );
}

function renderBody(entryJsons: readonly string[]): string {
  if (entryJsons.length === 0) {
    return '<p class="empty-state">no claims published yet</p>';
  }
  return `<ul class="claims">${entryJsons.map((json) => renderRow(parseEntry(json))).join("")}</ul>`;
}

/** Render the card HTML from the committed manifest entry strings. */
export function renderCard(entryJsons: readonly string[]): string {
  return (
    '<!doctype html><html lang="en"><head><meta charset="utf-8">' +
    "<title>openlore — published claims</title></head>" +
    `<body><main class="openlore-card"><h1>Published claims</h1>${renderBody(entryJsons)}</main></body></html>`
  );
}

/** `GET /` — the card as a 200 HTML response with the restrictive CSP. */
export function cardResponse(entryJsons: readonly string[]): Response {
  return new Response(renderCard(entryJsons), {
    status: 200,
    headers: {
      "content-type": "text/html; charset=utf-8",
      "content-security-policy": CARD_CONTENT_SECURITY_POLICY,
      "x-content-type-options": "nosniff",
    },
  });
}
