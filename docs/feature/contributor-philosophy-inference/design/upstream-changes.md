# Upstream Changes — contributor-philosophy-inference (DESIGN → product owner)

> From: Morgan (DESIGN, 2026-09-27). For review by Luna (nw-product-owner). None of these
> re-opens a user-locked decision (D-1..D-4, OD-CPI-1..7); they are clarifications or new edge
> cases DISCUSS did not cover. Proposed AC text is given so DISTILL can adopt it directly.

| # | Story | Change | Why (DESIGN evidence) | Proposed AC wording |
|---|---|---|---|---|
| UC-1 | US-CPI-001 | `--contributors 0` makes NO GitHub request (not "exactly 1 extra"). | N is applied after the one request; with N = 0 there is nothing to fetch (ADR-063 §2). | "`--contributors 0` records none, says so, and makes no contributors request." |
| UC-2 | US-CPI-001 | GitHub "contributor list too large" (403) and empty repo (204) are a NAMED NOTICE: no links, scrape otherwise succeeds, exit 0. | Not a rate-limit/auth/network failure the user can fix; failing the whole scrape would hide valid repo signals (DDD-14). | "When GitHub cannot list contributors for the repo, the CLI names the reason, records no links, and exits 0." |
| UC-3 | US-CPI-001 | `--contributors` on a USER target is rejected before any request. | The flag has no meaning for `scrape github <user>` (D-4: no crawl). | "`scrape github <user> --contributors N` is rejected with a usage error before any request." |
| UC-4 | US-CPI-002/004 | A hand-authored `adheresToPhilosophy` claim (evidence cites no supporting claim) counts as "already signed" and is never labelled STRONGER. | STRONGER requires knowing which support the earlier claim cited (ADR-064 §5). | "A person/philosophy pair I signed without inferred provenance is shown as already signed and never proposed for supersession." |
| UC-5 | US-CPI-004 | SUPPORT WEAKENED also fires when a cited supporting claim's author is no longer an ACTIVE subscription (not only on retraction), with the reason named. | D-2 eligibility = active peers; a peer removal withdraws that support exactly like a retraction (DDD-8). | "A signed inferred claim is flagged support weakened, naming the reason (retracted / peer no longer subscribed / not in local store), and is never modified." |
| UC-6 | US-CPI-001/002 | `github:` subjects match case-insensitively (a repo scraped as `burntsushi/ripgrep` joins a claim on `github:BurntSushi/ripgrep`). | GitHub names are case-insensitive; the scraper keeps the user-typed casing (DDD-5/7). | "Contribution links and repo claims whose `github:` subjects differ only in letter case are treated as the same repo." |
| UC-7 | US-CPI-003 | The compose preview shows the provenance as the evidence list of `at://…/org.openlore.claim/<cid>` + `https://github.com/<o>/<r>/commits?author=<login>` entries (plus the `derived-from` summary line). | ADR-064 wire encoding. | "The signed claim's evidence names each supporting claim by its AT-URI (author + CID) and the person's commits URL for each supporting repo." |
| UC-8 | US-CPI-002 | Candidate numbering is a function of (store state, `--person`, `--min-repos`): `--sign N` must be given the same filters as the list it refers to. | Filters change the numbered set (DDD-8). | "`infer people --sign N` numbers candidates identically to `infer people` run with the same filters." |

No story is split, dropped, or re-prioritized. No KPI changes.
