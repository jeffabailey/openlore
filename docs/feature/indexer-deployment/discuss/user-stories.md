<!-- markdownlint-disable MD024 -->
# User Stories: indexer-deployment

> Every story has a `job_id` from `docs/product/jobs.yaml`, or is `infrastructure-only` with a
> rationale. Personas, FR/NFR IDs and open questions are in `requirements.md`. AC IDs are
> collected in `acceptance-criteria.md`. Data is illustrative but realistic. Jeff's own DID is
> shown as `did:plc:jbailey5q8n`.

## System Constraints (cross-cutting, apply to every story)

- **C-1 Co-located, isolated (ADR-075).** The indexer runs on the PDS host as containers from a
  CI-built, signed arm64 image, pinned by digest. Containers get no AWS credentials, no `/pds`
  mount, a read-only root filesystem, a non-root user and no added capabilities (NFR-IXD-8).
- **C-2 Capability boundary (ADR-023).** The indexer is signing-incapable and holds no local store
  or secrets. The public surface is read-only search (FR-IXD-2).
- **C-3 No regression.** The PDS and the review app keep serving through every deploy and
  rollback (NFR-IXD-9). Memory and CPU budgets come from NFR-IXD-4/5.
- **C-4 Binary unchanged.** The ADR-077/078 behavior, exit codes 0/2/3 and events are used as
  they are. If a DESIGN mechanism for sharing the store (OQ-IXD-1) needs a code change, it is
  the only expected one.
- **C-5 Re-buildable index (WD-IXD-7).** The index is not backed up, and a rollback never needs
  a data restore.
- **C-6 Proportionate cost (NFR-IXD-6).** ≤ $2/month added. Sequencing with v1.7.0 is in
  `wave-decisions.md` (DEP-IXD-1).
- Technology choices are deferred to DESIGN/DEVOPS (OQ-IXD-1..9).

---

## US-IXD-001: Search the live public network index

- **job_id**: J-005 (sub-job J-005a; beneficiary J-009)
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1 | **Size**: 2-3 days

### Elevator Pitch

- **Before**: Maria runs `openlore search --object org.openlore.philosophy.reproducible-builds` and gets local-only results, because no network index runs anywhere.
- **After**: with `OPENLORE_INDEXER_URL=https://index.openlore.jeffbailey.us`, the same `openlore search` prints `Author: did:plc:priyaraman7x2k (priyaraman.bsky.social) … github:priyaraman/cargo-pin confidence 0.82 [self-attested]` next to Dmitri's claim from `pds.volkov.dev`.
- **Decision enabled**: Maria decides whether to look at `cargo-pin` or follow Priya (`openlore peer add`), based on authors she could not reach before.

### Problem

Maria wants to find developers who claim reproducible builds, but follows nobody who does. The
indexer that would answer her search is finished and tested, yet it runs nowhere. Her
`openlore search` therefore falls back to her local store, and Priya's claims on bsky.social
stay invisible to her.

### Who

- Maria | searcher at a laptop, no peers yet | wants a public index she can point the CLI at.
- Jeff | operator | wants the first deploy to leave the PDS and the review app untouched.

### Solution

Ship `openlore-indexer` as a CI-built, signed arm64 image. Run `serve` as a long-running service
on the PDS host behind Caddy at `index.openlore.jeffbailey.us`, with TLS and only the search
method exposed. Jeff does the first deploy by digest from his laptop.

### Domain Examples

#### 1: Happy path, authors on two PDSes

The index holds Priya's 3 self-attested claims (bsky.social) and Dmitri's 2 app-signed claims
(`pds.volkov.dev`). Maria searches the object `org.openlore.philosophy.reproducible-builds` and
sees Priya's `cargo-pin` claim and Dmitri's `nix-lockcheck` claim, each attributed to its own
author.

#### 2: Edge, before the first pass

Jeff has just deployed. No pass has run yet. Maria's search returns "no results" quickly. It is
not a server error. After the 14:15 pass, the same search returns Priya's claim.

#### 3: Error, someone tries to write

A scanner sends `POST https://index.openlore.jeffbailey.us/xrpc/com.atproto.repo.createRecord`
and `GET /admin`. Both are refused, and the index is unchanged.

#### 4: Boundary, plain HTTP

`http://index.openlore.jeffbailey.us/xrpc/org.openlore.appview.searchClaims?...` redirects to
HTTPS. The certificate is publicly trusted, so `curl` succeeds without `-k`.

### UAT Scenarios (BDD)

#### Scenario: Maria finds claims from authors on different PDSes through the public index

Given the public index holds Priya's self-attested claim on github:priyaraman/cargo-pin and Dmitri's app-signed claim on github:dvolkov/nix-lockcheck
And Maria has set OPENLORE_INDEXER_URL to https://index.openlore.jeffbailey.us
When Maria runs openlore search --object org.openlore.philosophy.reproducible-builds
Then Maria sees both claims, each attributed to its own author DID

#### Scenario: The public index is served over trusted TLS

Given the indexer is deployed
When Maria requests the search method over http and over https
Then the http request is redirected to https
And the https response uses a publicly trusted certificate for index.openlore.jeffbailey.us

#### Scenario: The public index offers search only

Given the indexer is deployed
When anyone sends a write request or requests any path other than the search method
Then the request is refused
And the index contents are unchanged

#### Scenario: A fresh deployment answers before its first pass

Given Jeff has just deployed and no pass has run
When Maria searches
Then she gets an empty result, not a server error

#### Scenario: The first deploy leaves the PDS and the review app serving

Given the PDS health check and the review app's /healthz answer 200
When Jeff deploys the indexer for the first time by digest
Then both still answer 200 during and after the deploy

### Acceptance Criteria

- [ ] AC-001.1 Search through the public URL returns claims from at least 2 distinct PDS hosts, each attributed to its author.
- [ ] AC-001.2 HTTPS uses a publicly trusted certificate, and HTTP redirects to HTTPS.
- [ ] AC-001.3 Only the search method (and an optional minimal health response) is reachable. Write methods and other paths are refused, and the index is unchanged.
- [ ] AC-001.4 Before the first pass, search returns an empty result and no 5xx.
- [ ] AC-001.5 The PDS `_health` and the review-app `/healthz` stay 200 through the first deploy.
- [ ] AC-001.6 The deployed image is the CI-built, signature-verified digest, and the containers meet C-1.

### Outcome KPIs

KPI-IXD-1 (see `outcome-kpis.md`).

### Technical Notes

- Depends on DEP-IXD-1 (the v1.7.0 Caddy sites mount and hop limit 1, then R-REPLACE).
- `openlore search` reads `OPENLORE_INDEXER_URL` / `[appview] indexer_url` today. Making the
  public index the default is OQ-IXD-6, out of scope.

---

## US-IXD-002: Keep the index refreshed every 15 minutes while search keeps answering

- **job_id**: J-005 (sub-job J-005a/J-005b; beneficiary J-009)
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1 | **Size**: 2-3 days

### Elevator Pitch

- **Before**: the index holds only what a manual pass found. Priya approves a new claim at 14:02, and Maria's `openlore search` never shows it.
- **After**: the 14:15 pass indexes it. At 14:16 Maria's `openlore search --subject github:priyaraman/cargo-pin` prints the new claim, `… confidence 0.74 [self-attested]`. A search at 14:15:30, while the pass is running, still answers.
- **Decision enabled**: Maria can trust that results reflect the network as of the last half hour, so she acts on what she sees instead of re-checking later.

### Problem

A public index is only useful if it keeps up. The pass is a one-shot command, and nothing runs
it on a schedule. The pass also writes the same store that search reads. DuckDB lets only one
process hold a file read-write, and a read-only open still conflicts (ADR-065). A naive setup
would fail one side every 15 minutes.

### Who

- Maria | searcher | wants fresh results without errors.
- Priya | author | wants her claims discoverable within half an hour.
- Jeff | operator | wants passes to run unattended and never pile up.

### Solution

Run `openlore-indexer ingest` every 15 minutes from a host timer, against the same index store
that `serve` reads. Store sharing is resolved so that neither side refuses the other (OQ-IXD-1).
Passes never overlap. The schedule resumes after a reboot or a replacement plus redeploy.

### Domain Examples

#### 1: Happy path

Priya approves a claim on `cargo-pin` at 14:02. The 14:15 pass lists 12 DIDs (11 `own_pds`,
1 skipped) in 9.8 s and indexes it. At 14:16 Maria sees it.

#### 2: Edge, search during a pass

Maria searches at 14:15:30 while the pass is writing. She gets results within 1 s (possibly
from before the pass). She gets no error and no "store busy".

#### 3: Edge, a slow pass

`pds.volkov.dev` hangs, and the pass takes 16 minutes (the worst case is about 13 min at 50 DIDs
under ADR-078 §4, plus margin). At 14:30 the timer does not start a second pass. The next pass
starts after this one ends.

#### 4: Error, the pass meets a busy store

Search is answering a burst of requests when the 14:45 pass starts. The pass still completes
with exit 0. It does not exit 2 with a store-lock error.

### UAT Scenarios (BDD)

#### Scenario: A newly approved claim becomes searchable within half an hour

Given Priya approved a claim on github:priyaraman/cargo-pin at 14:02
When the scheduled passes run
Then Maria finds that claim with openlore search no later than 14:32

#### Scenario: Search keeps answering while a pass runs

Given a pass is writing to the index
When Maria searches
Then she gets results within 1 second and no error

#### Scenario: A pass is never refused because search is busy

Given search is answering a burst of requests
When the scheduled pass starts
Then the pass completes with exit 0 and indexes the claims it listed

#### Scenario: Passes never overlap

Given a pass is still running when the next one is due
When the timer fires
Then no second concurrent pass starts
And the next pass runs after the current one ends

#### Scenario: The schedule survives a host restart

Given the host rebooted (or was replaced and the indexer redeployed)
When 15 minutes pass
Then a pass has run without Jeff starting it

### Acceptance Criteria

- [ ] AC-002.1 A claim published at time T is searchable by T+30 min (NFR-IXD-1).
- [ ] AC-002.2 Search during a pass answers in ≤ 1 s with no error (NFR-IXD-3).
- [ ] AC-002.3 A pass never exits 2 because search holds the store.
- [ ] AC-002.4 At most one pass runs at any time.
- [ ] AC-002.5 Passes resume unattended after a reboot, and after a replacement plus redeploy.

### Outcome KPIs

KPI-IXD-2, KPI-IXD-3.

### Technical Notes

- OQ-IXD-1 (store-sharing mechanism) and OQ-IXD-2 (container shape) are DESIGN's.
- The `main.rs` doc says `serve` also runs an ingest loop. Only one writer may exist (R-IXD-4).

---

## US-IXD-003: Add an author by editing the DID list, with no redeploy

- **job_id**: J-005 (sub-job J-005a; beneficiary J-009)
- **Release**: R2 Operate | **MoSCoW**: Must | **Priority**: P2 | **Size**: 1-2 days

### Elevator Pitch

- **Before**: adding Tomás Herrera (`did:plc:therrera2v6w`, bsky.social) to the index would need a new deploy, so his review-app claims stay unsearchable until Jeff ships something.
- **After**: Jeff runs `aws ssm put-parameter --name /openlore/prod/indexer/repo-dids --overwrite --value "<13 DIDs>"` at 15:05. The 15:15 pass logs `indexer.config.loaded {"repo_did_count":13,…}`, and at 15:16 Maria's `openlore search --contributor did:plc:therrera2v6w` lists his claims.
- **Decision enabled**: Maria can evaluate and follow Tomás. Jeff decides when coverage is complete, with no release ceremony.

### Problem

The set of authors the index covers changes as people join through the review app. If every
change needed a rebuild or redeploy, coverage would lag, and Jeff would batch changes or forget
them.

### Who

- Jeff | operator | wants to edit one value and be done.
- Maria | searcher | wants new authors to appear soon after they join.

### Solution

The DID list is a static SSM parameter. The host turns it into a read-only file before each pass.
The pass reads it, so an edit takes effect on the next pass. A failed read keeps the last good
list. A malformed list refuses the pass with exit 2, which alerts through US-IXD-004.

### Domain Examples

#### 1: Happy path, add Tomás

Jeff adds `did:plc:therrera2v6w` at 15:05. The 15:15 pass reports `repo_did_count` 13. Tomás's 2
self-attested claims are searchable at 15:16. Nothing was redeployed or restarted.

#### 2: Edge, remove Dmitri

Jeff removes `did:plc:dvolkov3m9q`. Later passes no longer list him. His 2 already-indexed claims
stay searchable (ADR-024 has no delete). Purging them is OQ-IXD-9.

#### 3: Error, typo

Jeff saves `did:plc:therrera2v6w,tomas`. The 15:15 pass is refused at startup with exit 2,
naming `OPENLORE_INDEXER_REPO_DIDS` and the entry `tomas`. Jeff is emailed. Search keeps serving
the 15:00 index. Jeff fixes the value, and the 15:30 pass exits 0.

#### 4: Error, SSM is unreachable

At 16:00 the host cannot read the parameter. The 16:00 pass runs with the last good list of 13
DIDs, not an empty list.

### UAT Scenarios (BDD)

#### Scenario: A DID added to the list is indexed on the next pass without a redeploy

Given Jeff adds did:plc:therrera2v6w to the DID list parameter at 15:05
When the 15:15 pass runs
Then the pass reports 13 configured DIDs
And Maria finds Tomás's claims with openlore search
And no deploy or restart happened

#### Scenario: Removing a DID stops listing it but keeps its indexed claims searchable

Given Dmitri's 2 claims are indexed
When Jeff removes did:plc:dvolkov3m9q from the list and the next pass runs
Then Dmitri is not listed in that pass
And his 2 claims are still searchable

#### Scenario: A malformed list refuses the pass, names the bad entry and keeps search serving

Given Jeff saves the list "did:plc:therrera2v6w,tomas"
When the next pass runs
Then the pass exits 2 with a message naming OPENLORE_INDEXER_REPO_DIDS and "tomas"
And search keeps returning the claims indexed before the edit

#### Scenario: An unreadable list never empties the pass

Given the last good list has 13 DIDs
And the host cannot read the parameter at pass time
When the pass runs
Then it uses the 13 DIDs from the last good list

### Acceptance Criteria

- [ ] AC-003.1 A list edit takes effect on the next pass, with no deploy and no container restart.
- [ ] AC-003.2 A removed DID is no longer listed, and its indexed claims remain.
- [ ] AC-003.3 A malformed list gives exit 2, names the bad entry, and leaves the existing index searchable.
- [ ] AC-003.4 A read failure uses the last good list. The pass never runs with an empty list because of a read failure.
- [ ] AC-003.5 The container never receives AWS credentials to read the list (C-1).

### Outcome KPIs

KPI-IXD-4.

### Technical Notes

- OQ-IXD-3 covers how the list is rendered (parameter type, keeping the last good copy). The host
  role gets read access to `/openlore/prod/indexer/*` only.
- The list is not secret (public DIDs).

---

## US-IXD-004: Be emailed only when the indexer is really broken

- **job_id**: infrastructure-only
- **infrastructure_rationale**: Operator alerting has no searcher-facing surface. It protects
  the J-005 outcome (a live, fresh index) by making total outages and local faults visible
  within a pass or two. Its release slice (R2) also contains US-IXD-003 (J-005).
- **Release**: R2 Operate | **MoSCoW**: Must | **Priority**: P2 | **Size**: 1 day

### Elevator Pitch

- **Before**: if PLC goes down or Jeff's DID-list typo stops every pass, nobody notices until Maria complains that search is stale.
- **After**: Jeff gets an SNS email "ALARM: openlore-indexer-total-outage" after the second consecutive exit-3 pass, or "ALARM: openlore-indexer-pass-refused" after any exit-2 pass. An "OK" email follows on recovery.
- **Decision enabled**: Jeff decides whether to act now (exit 2: fix config, disk or image) or wait out an upstream outage (exit 3).

### Problem

ADR-078 gives distinct exit codes: 3 means every remote source failed (usually transient) and 2
means a local fault (config, probe, store). With no supervisor watching them, both go
unnoticed. Alerting on every partial skip would be noise, because one third-party PDS is down
on a routine basis.

### Who

- Jeff | solo operator, email on his phone | wants few, actionable alerts.

### Solution

Two alarms on the existing SNS topic, with recovery notifications: 2 consecutive exit-3 passes,
and any exit-2 pass. Exit 0, including a partial skip, never alarms.

### Domain Examples

#### 1: Total outage

PLC is down from 02:00 to 02:40, and no fallback is configured. The 02:00 and 02:15 passes exit
3. One email arrives after the 02:15 pass. The 02:45 pass exits 0, and an OK email arrives.

#### 2: One blip

Only the 03:00 pass exits 3. The 03:15 pass exits 0. No email.

#### 3: Partial skip, all day

`pds.volkov.dev` returns 502 all day. Every pass exits 0 with `skipped: 1`. No email. The skip is
visible in the logs (US-IXD-005).

#### 4: Local fault

The data volume fills, and the 11:30 upsert fails, so the pass exits 2. One email arrives after
that pass.

### UAT Scenarios (BDD)

#### Scenario: Two consecutive total outages alert the operator once

Given every DID is unreachable
When two consecutive passes exit 3
Then Jeff receives one alarm email
And when a later pass exits 0 Jeff receives a recovery email

#### Scenario: A single total-outage pass does not alert

Given one pass exits 3
When the next pass exits 0
Then no alarm email is sent

#### Scenario: Any refused or failed pass alerts the operator

Given Jeff saved a malformed DID list
When the next pass exits 2
Then Jeff receives an alarm email after that pass

#### Scenario: Partial skips never alert

Given pds.volkov.dev is down all day
When every pass exits 0 with one DID skipped
Then no alarm email is sent

### Acceptance Criteria

- [ ] AC-004.1 Two consecutive exit-3 passes produce exactly one alarm email. A single exit 3 produces none.
- [ ] AC-004.2 Any exit-2 pass produces an alarm email within 15 minutes of the pass ending.
- [ ] AC-004.3 Exit 0, with or without skips, never alarms.
- [ ] AC-004.4 Recovery sends an OK notification. Alarms use the existing SNS topic, and no new subscription is created.
- [ ] AC-004.5 Each alarm fired once in a deliberate test and returned to OK before go-live.
- [ ] AC-004.6 The added alarm cost is ≤ $0.30/month.

### Outcome KPIs

KPI-IXD-5.

### Technical Notes

- A dead timer or a dead `serve` produces no exit codes, so neither alarm sees it (OQ-IXD-4,
  needs a user decision). US-IXD-005 makes it visible by hand.
- The review-app pattern applies: log metric filters plus alarms, `ok_actions`, and an
  `*_alarms_enabled` toggle.

---

## US-IXD-005: See when the last good pass ran

- **job_id**: infrastructure-only
- **infrastructure_rationale**: Operator diagnostics with no searcher-facing surface (public
  freshness is OQ-IXD-5). They make J-005 freshness checkable and cover the gap left by the
  alarms (a dead timer). Its slice (R2) contains US-IXD-003 (J-005).
- **Release**: R2 Operate | **MoSCoW**: Should | **Priority**: P3 | **Size**: 1 day

### Elevator Pitch

- **Before**: to know whether the index is fresh, Jeff would SSM into the host and read container logs by hand.
- **After**: Jeff runs one laptop command (DESIGN names it) and sees `last successful pass: 2026-10-06T14:45:12Z (11 min ago) configured 12, own_pds 11, fallback 0, skipped 1 (did:plc:dvolkov3m9q pds_unreachable), 9.8 s`, plus the exit codes of the last 8 passes.
- **Decision enabled**: Jeff decides whether the index is healthy, or whether he must restart the timer, fix a DID, or wait for an upstream recovery.

### Problem

The alarms cover outages and local faults, but not a timer that has stopped. They also do not
show which DIDs are being skipped. Jeff needs one place to see freshness and recent outcomes.

### Who

- Jeff | operator at his laptop with AWS SSO | wants the answer in one command, in under 10 s.

### Solution

Expose the last successful pass time, its `pass_summary` and `source_skipped` details, and the
recent exit codes through one laptop command, using data that the passes already emit.

### Domain Examples

#### 1: Healthy

The last success was at 14:45:12Z, 11 minutes ago, with 1 skip (Dmitri, `pds_unreachable`).

#### 2: Stale after outages

The last 12 passes exited 3. The output shows "last successful pass: 3 h ago" and lists 12 exit
codes of 3.

#### 3: Dead timer

No pass has run since 09:00. The output shows "last pass of any kind: 2 h 10 min ago", which is
Jeff's cue that the schedule stopped.

### UAT Scenarios (BDD)

#### Scenario: The operator sees the last successful pass and its counts

Given the 14:45 pass exited 0 with 11 own_pds and 1 skipped DID
When Jeff runs the freshness command at 14:56
Then he sees 14:45:12Z, "11 min ago", the counts, and the skipped DID with its reason

#### Scenario: Stale index after repeated outages is obvious

Given the last 12 passes exited 3
When Jeff runs the freshness command
Then he sees the last successful pass is 3 hours old and the 12 recent exit codes

#### Scenario: A stopped schedule is visible

Given no pass has run for 2 hours
When Jeff runs the freshness command
Then he sees how long ago the last pass of any kind ran

### Acceptance Criteria

- [ ] AC-005.1 One laptop command shows the last successful pass time, its age, and the `pass_summary` counts.
- [ ] AC-005.2 It lists the skipped DIDs with reasons for that pass, and the exit codes of recent passes.
- [ ] AC-005.3 It shows the age of the last pass of any kind, so a stopped schedule is visible.
- [ ] AC-005.4 It answers in ≤ 10 s, needs no interactive host shell, and shows no claim content (WD-105).

### Outcome KPIs

KPI-IXD-3 (measured with it).

### Technical Notes

- The existing `stats` verb reports ingest lag. Whether to reuse it or query the logs is DESIGN's choice.

---

## US-IXD-006: Ship a new indexer version, or roll back, without disturbing the PDS

- **job_id**: infrastructure-only
- **infrastructure_rationale**: The deploy and rollback mechanics, and the host resource gate,
  have no searcher-facing surface. They protect J-005 availability and the PDS that every
  OpenLore author depends on. The slice (R2) contains US-IXD-003 (J-005).
- **Release**: R2 Operate | **MoSCoW**: Must | **Priority**: P2 | **Size**: 2 days

### Elevator Pitch

- **Before**: there is no deploy path for the indexer, so a fix to the binary cannot reach production, and a bad version could not be undone.
- **After**: Jeff runs `deploy/indexer/deploy.sh deploy <40-hex sha>`. It prints the verified digest and `serve ready`, and Maria's next search answers. If a version misbehaves, `deploy.sh rollback` restores the previous digest in one command.
- **Decision enabled**: Jeff decides to ship fixes as often as needed, because a bad release is one command from undone and cannot take down the PDS.

### Problem

The PDS host is a t4g.micro with 1 GiB, shared with the PDS and the review app. A memory-hungry
pass, or a broken image, must not degrade Jeff's PDS. Without a by-digest deploy and rollback,
every change is risky.

### Who

- Jeff | operator | wants fearless, repeatable deploys.

### Solution

A deploy script in the review-app style: it verifies the CI build and the signature, deploys by
digest, keeps the release history, and rolls back to the previous digest automatically when the
new one is not ready, or on command. Resource caps and a hard memory re-measure gate apply, with
t4g.small as the fallback.

### Domain Examples

#### 1: Routine upgrade

Jeff deploys sha `4f2c…` (digest `sha256:9b1e…`). Search is down ≤ 30 s. A pass in progress is
not cut short by the deploy, or if it is, the next pass recovers with no data loss.

#### 2: Bad image

The new digest refuses to start (exit 2, probe failure). It is not ready within the deadline, so
the script restores the previous digest. Jeff also receives the US-IXD-004 email.

#### 3: Memory gate fails

During the re-measure, host `MemAvailable` drops to 90 MB while a pass and a review-app scan run
together. Jeff moves to t4g.small (an in-place stop/start, +$6.13/month).

#### 4: Unsigned image

Jeff passes a digest that CI did not sign. The script refuses before anything changes on the host.

### UAT Scenarios (BDD)

#### Scenario: A new version is deployed by digest without disturbing the PDS

Given the indexer runs digest A and the PDS health check answers 200
When Jeff deploys the CI-built, signed digest B
Then search answers from digest B within 30 seconds of the switch
And the PDS and the review app answered 200 throughout

#### Scenario: A version that does not become ready is rolled back automatically

Given Jeff deploys digest B, which refuses to start
When it is not ready within the deadline
Then digest A is running again and search answers

#### Scenario: The operator rolls back on command

Given digest B is running
When Jeff runs the rollback command
Then digest A is running and search answers, with no data restore

#### Scenario: An unverified image is refused

Given a digest that CI did not build and sign
When Jeff tries to deploy it
Then the deploy is refused and nothing changes on the host

#### Scenario: The host keeps headroom with every workload running

Given a pass, a review-app scan and search traffic run at the same time
When Jeff reads the re-measure signals
Then the indexer peak is ≤ 256 MB, MemAvailable stays above 128 MB, swap-in is about 0, and the PDS answers 200

#### Scenario: A failed headroom gate moves the host to the larger size before go-live

Given the re-measure shows MemAvailable at 90 MB during a pass and a review-app scan
When Jeff applies the t4g.small fallback and repeats the re-measure
Then the gate passes and go-live proceeds

### Acceptance Criteria

- [ ] AC-006.1 Deploys accept only a CI-built, signature-verified digest, and pin it so that a reboot restarts the same digest.
- [ ] AC-006.2 A not-ready digest is rolled back automatically. A manual rollback is one command and needs no data restore.
- [ ] AC-006.3 Search downtime per deploy is ≤ 30 s, and the PDS and review app stay at 200 throughout (NFR-IXD-9).
- [ ] AC-006.4 The re-measure gate (NFR-IXD-4) passed on the host, or t4g.small was adopted, before go-live.
- [ ] AC-006.5 The containers meet C-1, and there are resource caps that make the kernel kill an indexer process before the PDS.

### Outcome KPIs

KPI-IXD-6.

### Technical Notes

- The first deploy is part of US-IXD-001. This story adds the routine-deploy, rollback and resource-gate guarantees.
- 24 h of `CPUCreditBalance` observation is part of the gate (NFR-IXD-5).
