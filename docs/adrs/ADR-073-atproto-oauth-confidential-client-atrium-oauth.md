# ADR-073: ATProto OAuth as a Confidential Web Client via `atrium-oauth`, Create-Only Write Port

- **Status**: Accepted (2026-10-04) — implemented; see `docs/evolution/bluesky-claim-review-app-evolution.md` (proposed 2026-10-04). SPIKE-2 passed with `atrium-oauth` 0.1.7 (with workarounds for its revoke-200 and callback `todo!()` defects; `hickory` ≥ 0.26); SPIKE-3 passed on bsky.social, so granular scopes are used (OpenLore PDS run pending).
- **Date**: 2026-10-04
- **Deciders**: Jeff Bailey (sign-in via ATProto OAuth, settled), Morgan (nw-solution-architect)
- **Feature**: bluesky-claim-review-app (DESIGN), resolves OD-BRA-2 (client type and library) and OD-BRA-7 (scopes)

## Context

Sign-in is settled: users sign in with a Bluesky handle through ATProto OAuth. What the ATProto
OAuth spec (https://atproto.com/specs/oauth, read 2026-10-04) requires of us:

- `client_id` is the HTTPS URL of a client-metadata JSON document.
- PAR, PKCE S256 and DPoP (ES256, server nonces) are mandatory.
- The client must check that the token response's `sub` matches the expected DID.
- Confidential clients authenticate with `private_key_jwt` (ES256, `jwks_uri`). Their sessions
  can be unlimited, with each refresh token lasting up to 180 days. Public clients are capped at a
  2-week session.

Project constraints:

- rustls only (`openssl` and `openssl-sys` are banned).
- The license allowlist excludes MPL and every GPL variant.
- Sources are crates.io only.
- No web framework.
- Time to market and low operations cost are the top quality attributes.

Granular permission scopes (https://atproto.com/specs/permission) such as
`repo:<nsid>?action=create` were live on bsky.social as of 2025-08, and were "rolling out to
self-hosted PDS distributions". Bluesky advised holding off on using them in production apps
(https://github.com/bluesky-social/atproto/discussions/4118). Their status in late 2026 is
unverified. Incremental (step-up) authorization is not specified.

## Decision

1. **Client type: confidential web client.**
   - `client_id` is `${app_origin}/oauth/client-metadata.json`, with
     `token_endpoint_auth_method=private_key_jwt`, `jwks_uri=${app_origin}/oauth/jwks.json`, and
     redirect URI `${app_origin}/oauth/callback`.
   - One ES256 client key, held in SSM, is loaded at startup and never logged.
   - A confidential client gives users long-lived sessions (less re-consent friction, which serves
     the KPI-BRA-2 time-to-publish goal), and a server-side web app can keep a secret.
2. **Library: `atrium-oauth` 0.1.x (MIT), with `atrium-identity` and `atrium-api`, built with
   `default-features = false`.**
   - The adapter supplies its own `HttpClient` over the workspace `reqwest 0.12` with
     `rustls-tls-webpki-roots`. The library's default `default-client` feature enables reqwest's
     native-tls, which `deny.toml`'s `openssl-sys` ban would reject anyway.
   - Its generic `StateStore` and `SessionStore` traits are implemented over a port backed by the
     encrypted private store (ADR-074).
   - Sources: https://crates.io/crates/atrium-oauth (0.1.7, 2026-03-26); its source code shows
     `private_key_jwt` with a keyset, DPoP nonce retry, `refresh()`, `revoke()`, and issuer/`sub`
     verification.
3. **Write capability = a create-only port.**
   - `UserRepoWritePort` exposes **create** for exactly two collections, through a closed
     `WritableCollection` ADT: `org.openlore.claim` and `app.bsky.feed.post`.
   - There is no update, put, delete or `applyWrites` method. I-BRA-8 ("never modify or delete
     published claims") is therefore non-representable. A soft retraction is itself a *create*.
   - Writes go to the PDS bound to the OAuth session. That is the PDS from the user's DID document
     (I-BRA-7), carried by the session and never supplied by the client.
4. **Scopes (OD-BRA-7).**
   - **Target**: `atproto repo:org.openlore.claim?action=create repo:app.bsky.feed.post?action=create`.
     This matches NFR-BRA-3 exactly, and the authorization server then enforces I-BRA-8 as well.
   - **Fallback, if SPIKE-3 shows either PDS (bsky.social or the pinned OpenLore PDS image)
     rejects it**: `atproto transition:generic`. In that case:
     - the create-only port remains the enforcement layer;
     - the landing page discloses that "your PDS may describe this as broad access; the app can
       only create claims and, if you choose, one post";
     - the deviation is recorded as an accepted NFR-BRA-3 risk.
   - Scopes are configuration. Changing them later needs no code change, only a re-authorization.
   - **No incremental scope request.** Post permission is requested at sign-in, and the landing
     page states it plainly ("never posts unless you press Post"). Incremental authorization is
     not specified by atproto, so planning on it would block D-11.
5. **The DID is pinned (AC-001.6).**
   - The handle is resolved to a DID before PAR.
   - After the callback, the session's `sub` must equal that DID, or sign-in is refused. This is a
     pure check in `review-domain`.
6. **Sign-out and disconnect revoke the grant** at the authorization server (best-effort, logged
   on failure). Private app-side state survives sign-out and is purged only by disconnect.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| **Public client** (browser-held tokens or a server acting as a public client) | Rejected. Public sessions are capped at 2 weeks, forcing more re-consent. Browser-held tokens would expose write tokens to page scripts (NFR-BRA-2). |
| **`jacquard-oauth`** (the most active Rust ATProto OAuth crate) | Rejected. It is MPL-2.0, which is not on the `deny.toml` allowlist (ADR-012). Allowing MPL would need its own ADR, and the gain does not justify that. |
| **`atproto-oauth` (Gerakines, MIT)** | **Fallback** if SPIKE-2 fails. It provides building blocks (PKCE, DPoP, JWK, `private_key_jwt`), but we would wire PAR, the callback and refresh ourselves, which costs several more days. |
| **Hand-rolled OAuth (PAR + DPoP + `private_key_jwt` on `p256` + `reqwest`)** | Rejected for time to market. The security-critical code (nonce handling, JWT construction, issuer verification) would need its own test corpus. Kept only as a last resort. |
| **App passwords** | Rejected. That is today's CLI path, which J-009 exists to remove. The app would also hold a full-account credential. |

## Consequences

- **Positive**:
  - The approach complies with the spec, holds no full-account credential, and offers long
    sessions.
  - Least privilege is enforced at two layers (port shape, plus server scopes when granular scopes
    are available).
- **Negative / risks**:
  - `atrium-oauth` is 0.1.x with a slow release cadence (API churn risk). It pulls in `dashmap`,
    `trait-variant`, `jose-*` and `hickory-*`, so the whole tree needs `cargo deny`.
  - The fallback scope (`transition:generic`) over-grants at the server.
  - Key rotation for the client JWK needs a runbook (DEVOPS): publish both keys in the JWKS, then
    retire the old one.
- **Contract tests**: the PDS authorization server (PAR, token, revoke) and the client-metadata
  fetch are external integrations. See the handoff annotation in `architecture-design.md`.
