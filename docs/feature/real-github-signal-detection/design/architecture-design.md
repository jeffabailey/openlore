<!-- markdownlint-disable MD013 -->
# Architecture Design — real-github-signal-detection

> DESIGN wave. Paradigm: functional Rust (ADR-007). Scope: make the GitHub
> scraper detect its signals from REAL public repo metadata, replacing the
> slice-02 walking-skeleton harvest that read a synthetic `signals[]` field only
> the `FakeGithub` test double provides. Lean density.

## 1. Problem

Today `adapter-github::harvest_repo` fetches `GET /repos/{owner}/{repo}` and calls
`client::parse_signals(&body)`, which reads a `signals[]` array from the response.
The **real** GitHub API returns no such field, so a live scrape of any real repo
yields **0 signals** and therefore 0 candidates (verified:
`openlore scrape github BurntSushi/ripgrep` → "0 signals"). The signal-DETECTION
logic — computing which of the 5 bounded `SignalKind`s a real repo exhibits — was
never implemented (slice-02 walking skeleton; `adapter-github/src/lib.rs:41`
"live paths land Phase 03/04"). This feature implements that detection.

The 5 bounded signals (`ports::github::SignalKind`) and their real GitHub sources:

| SignalKind | Real source | Cost |
|---|---|---|
| **MemorySafetyLanguage** | `language` field of `GET /repos/{owner}/{repo}` | **0 extra calls** (already fetched) |
| DependencyManifestPinned | `GET /repos/{o}/{r}/contents/Cargo.lock` (200 = present) | 1 call |
| SemverAndChangelog | `GET /repos/{o}/{r}/tags` (semver) + contents CHANGELOG | 1–2 calls |
| DocsPresentAndSubstantial | `GET /repos/{o}/{r}/readme` (`size`) + `contents/docs` | 1–2 calls |
| TestRatioOrCiMatrix | `GET /git/trees/{branch}?recursive=1` (test/src ratio) or `.github/workflows` | 1–2 calls |

## 2. Functional decomposition (pure core + effect shell)

The scraper stays split (WD-56): the adapter FETCHES + reshapes raw facts; the pure
domain DECIDES which signals fire.

```
adapter-github (EFFECT)                     scraper-domain (PURE)
  harvest_repo:                               detect_signals(&RepoFacts) -> Vec<Signal>
    body = GET /repos/{o}/{r}                   MEMORY_SAFE_LANGUAGES (curated set)
    facts = parse_repo_facts(&body)  --RepoFacts-->  (business rule: which languages
    signals = detect_signals(&facts)                  embody the memory-safety philosophy)
    signals.extend(parse_signals(&body))  // legacy bridge, see §4
    -> Vec<Signal>
```

- **`RepoFacts`** (new, in `scraper-domain`): the structured facts detection needs.
  Walking skeleton: `{ language: Option<String>, source_url: String }`. Extended
  per later slice (`has_cargo_lock: bool`, `readme_bytes: Option<u64>`, `tags:
  Vec<String>`, `has_changelog: bool`, `test_file_count`/`src_file_count`, …).
- **`detect_signals(&RepoFacts) -> Vec<Signal>`** (new, `scraper-domain`): pure,
  total; a sibling of `derive_candidates`. Each detector is one pure predicate over
  `RepoFacts`. Walking skeleton implements ONLY the `MemorySafetyLanguage` arm.
- **`MEMORY_SAFE_LANGUAGES`** (new, `scraper-domain`): curated const set — the
  garbage-collected / ownership-safe languages that embody the memory-safety
  philosophy: Rust, Go, Swift, Kotlin, Java, C#, Python, Ruby, Scala, Haskell,
  Elixir, Erlang, OCaml, Clojure. EXCLUDES C, C++, assembly, unsafe-by-default.
  Match is case-insensitive against the GitHub `language` string.
- **`parse_repo_facts(&Value) -> RepoFacts`** (new, `adapter-github::client`): pure
  reshape of the real `/repos` JSON (reads `language`, `html_url`), mirroring the
  existing pure `parse_signals` / `parse_auth_report`.

### Dependency edge

`adapter-github` gains a dependency on `scraper-domain` (for `RepoFacts` +
`detect_signals`). This is architecturally sound — an EFFECT adapter depending on a
PURE domain crate (dependencies point inward); no cycle (`scraper-domain` does not
depend on `adapter-github`). `scraper-domain` stays pure (no I/O crate reaches it).
`xtask check-arch`: the composition-root rule restricts who depends on `adapter-*`
(only `cli`/`openlore-indexer`) — it does NOT restrict an adapter depending on a
domain crate. Expected to stay green at 21 members; verify in DELIVER.

## 3. Honest signal semantics

`MemorySafetyLanguage`'s SSOT description is "Primary language is Rust OR a
memory-safety language + no unsafe blocks". The walking skeleton detects the
LANGUAGE half only (primary language ∈ the safe set); the "no unsafe blocks" refinement
(needs a code scan / GitHub code-search) is DEFERRED. The emitted `Signal.value` is
honest about what was measured — e.g. `"primary language: Rust"` — never claiming
"no unsafe" was verified. Confidence stays the mapping default `0.25` (speculative;
WD-52 / I-SCR-3) — a heuristic, human-signed.

## 4. The legacy `signals[]` bridge (transitional)

The synthetic `signals[]` field is a walking-skeleton scaffold the real API never
provides; the whole existing scrape acceptance suite (`scrape_candidates.rs`,
`scrape_sign.rs`, …) injects signals through it via `FakeGithub`. To avoid a
big-bang test migration, `harvest_repo` UNIONS both paths:
`detect_signals(parse_repo_facts(body))` ∪ `parse_signals(body)`. Existing fake
bodies carry no `language` field → `detect_signals` returns `[]` → those tests are
untouched; real repos (and the new walking-skeleton test) carry `language` but no
`signals[]` → detection fires. As detectors 2–5 land and the fake fixtures migrate to
realistic bodies, a final cleanup slice removes the `parse_signals` path (the
synthetic field disappears entirely). Dedup by `SignalKind` guards the (currently
impossible) both-present case.

## 5. Slicing (carpaccio — walking skeleton first)

| Slice | Detector | New endpoints | Notes |
|---|---|---|---|
| **RGSD-1 (walking skeleton)** | MemorySafetyLanguage | none | `RepoFacts{language}` + `detect_signals` (language arm) + `parse_repo_facts` + harvest union + FakeGithub `language` posture. A real scrape of a Rust repo yields the `memory-safety` candidate end-to-end. |
| RGSD-2 | DependencyManifestPinned | `contents/Cargo.lock` | + `RepoFacts{has_cargo_lock}` |
| RGSD-3 | SemverAndChangelog | `tags` + CHANGELOG contents | semver parse of tag names |
| RGSD-4 | DocsPresentAndSubstantial | `readme` + `contents/docs` | README byte threshold (>200 lines ≈ size heuristic) |
| RGSD-5 | TestRatioOrCiMatrix | `git/trees?recursive=1` or `.github/workflows` | test/src file ratio |
| RGSD-6 (cleanup) | — | — | migrate fake fixtures to realistic bodies; remove the `signals[]`/`parse_signals` scaffold |

Each slice adds one pure detector arm + its `RepoFacts` field + the effect fetch +
its FakeGithub posture; each is independently dogfoodable against a real repo.

## 6. Rate-limit budget

Walking skeleton adds ZERO calls (reuses the resolve/harvest body). Later detectors
add 1–2 calls each; the full 5-detector harvest is ≤ ~6 calls/repo — trivial against
the 5000/hr authenticated budget, acceptable against 60/hr anon for interactive use.
No change to the existing `GithubError::RateLimited` handling.

## 7. Testing strategy

- PURE unit (scraper-domain): `detect_signals` property tests — every language in
  `MEMORY_SAFE_LANGUAGES` (any case) yields exactly one `MemorySafetyLanguage`
  signal; every out-of-set / `None` language yields none. `RepoFacts` in / `Vec<Signal>` out — no I/O.
- PURE unit (adapter-github): `parse_repo_facts` reshapes a real-shaped `/repos`
  body (with `language`) correctly; a body lacking `language` → `None`.
- Acceptance (subprocess + FakeGithub): a new FakeGithub posture serving a realistic
  `/repos` body (`language: "Rust"`, NO synthetic `signals[]`) → `openlore scrape
  github <repo>` derives the `org.openlore.philosophy.memory-safety` candidate; a
  C++/`None` posture → that candidate is absent. Existing scrape ATs stay green
  (union bridge).

## 8. Handoff

**To DISTILL:** author the RGSD-1 acceptance test (FakeGithub `language` posture →
memory-safety candidate; + a non-safe-language negative) + the pure unit RED for
`detect_signals` / `parse_repo_facts`. **To DELIVER:** functional crafter — pure
`detect_signals`/`RepoFacts`/`MEMORY_SAFE_LANGUAGES` in `scraper-domain`,
`parse_repo_facts` + harvest union in `adapter-github` (+ the `scraper-domain` dep),
FakeGithub posture in `test-support`. No new crate; check-arch stays 21.
