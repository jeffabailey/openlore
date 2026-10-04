# Acceptance Criteria: bluesky-claim-review-app

Derived from the UAT scenarios in `user-stories.md`. They are solution-neutral and
observable. `@property` marks invariants that must hold continuously.

## Cross-cutting invariant properties

```gherkin
@property @I-BRA-1
Scenario: Pending suggestions stay private until approved
  Given any user has pending suggestions
  Then no pending suggestion is written to any PDS
  And no pending suggestion appears in a share post, a profile page, OpenLore search/AppView, or any feed
  And no other signed-in user and no anonymous visitor can read that user's queue

@property @I-BRA-2
Scenario: Declines leave no public trace
  Given any user declines a suggestion
  Then nothing is written to their PDS or any public surface
  And only that user can see the decline

@property @I-BRA-3
Scenario: Every write follows an explicit confirm
  Then every claim, retraction and post written to a user's PDS corresponds to that user's confirm action after a preview

@property @I-BRA-4
Scenario: No scrape without current ownership
  Then every scrape is immediately preceded by a passing check that the signed-in DID is in the GitHub bio

@property @I-BRA-5
Scenario: Self-attested claims are first-class
  Then every claim approved via the app is accepted by OpenLore's read/verify path and shown as "self-attested", never "unverified"
```

## Per story

| AC | Criterion | From scenario |
|----|-----------|---------------|
| AC-000.1 | Client metadata is served over public HTTPS on `${app_origin}` and names "OpenLore review" | Bluesky can identify the review app |
| AC-000.2 | HTTP redirects to HTTPS | Landing page reachable over HTTPS only |
| AC-000.3 | Metadata outage → "temporarily unavailable, nothing changed" | Unavailability is explained |
| AC-001.1 | Valid handle → "Signed in as @<handle>" | Priya signs in |
| AC-001.2 | The three "never" commitments are shown before input | What the app will never do |
| AC-001.3 | Works for self-hosted PDSes | Dmitri signs in |
| AC-001.4 | Unresolvable handle → actionable message | Mistyped handle |
| AC-001.5 | Cancel or deny → "No access granted. Nothing changed.", no session | Cancelling changes nothing |
| AC-001.6 | Session DID == handle-resolved DID, else refused | (integration checkpoint) |
| AC-001.7 | Sign out ends the session | (journey) |
| AC-002.1 | The exact signed-in DID is shown with copy, bio location and reason | Exact DID to copy |
| AC-002.2 | Pass only if the bio contains the signed-in DID as an exact token | Priya proves her account |
| AC-002.3 | Distinct messages: user not found / DID not found / different DID / rate-limited with wait. Retry allowed | Sam; Dmitri; rate limit |
| AC-002.4 | No scan and no suggestion while unverified | Sam cannot claim |
| AC-002.5 | The link is per DID. DID A's verification never authorizes DID B | Link belongs to one DID |
| AC-002.6 | A handle is never accepted as proof | (D-3) |
| AC-003.1 | Ownership is re-verified immediately before each scan. Failure blocks the scan | Re-checked before scan |
| AC-003.2 | Cards show subject, philosophy, confidence plus bucket, signal and evidence | Evidence-backed suggestions |
| AC-003.3 | "Private: only you can see these" is always shown | (I-BRA-1) |
| AC-003.4 | Only the owner DID can read its queue. Others and anonymous visitors get not-found | Only Priya can see |
| AC-003.5 | Pending → no PDS write, post, profile, search, AppView or feed exposure | Pending never exposed |
| AC-003.6 | Empty-result guidance | Empty result guides Aisha |
| AC-003.7 | Per-repo progress. Rate-limit partial results kept, resume offered | (failure mode) |
| AC-003.8 | Keyboard triage with visible focus | (NFR-BRA-6) |
| AC-004.1 | Write only after explicit confirm on the preview | Preview shows exactly |
| AC-004.2 | Written to the PDS from the user's DID document | Publish into own PDS |
| AC-004.3 | **Approved claim accepted by OpenLore's read/verify path, shown as self-attested** | OpenLore accepts as self-attested |
| AC-004.4 | Preview == record field-for-field. Confidence = round(v × 10000). No bucket | Publish into own PDS |
| AC-004.5 | Preview has "not as truth". Confirmation shows the at:// URI and retract path | Preview; publish |
| AC-004.6 | Back writes nothing. A failed write leaves it pending, with no partial record and a retry | Backing out; failed publish |
| AC-004.7 | An approved item leaves the queue and is counted | (journey counts) |
| AC-005.1 | Swap to any vocabulary entry. Confidence 0.00–1.00 in steps of 0.01 | Publishes edited claim |
| AC-005.2 | Edits are reflected in the preview and the record | Publishes edited claim |
| AC-005.3 | Bucket displayed, never written | Bucket a label |
| AC-005.4 | Invalid input → guidance when the field loses focus, approval blocked | Out-of-range |
| AC-005.5 | Cancel restores the original | Cancelling an edit |
| AC-006.1 | "Not me" → private-decline confirmation with Undo | Declining writes nothing |
| AC-006.2 | **No record of any kind written to the PDS on decline** | Declining writes nothing |
| AC-006.3 | Declines are visible only to the owner, never on profile, search, AppView, feeds or posts | Nobody else sees |
| AC-006.4 | A declined key is never re-offered | Not offered again |
| AC-006.5 | Undo → pending | Undo restores |
| AC-007.1 | Profile lists published, non-retracted claims, labelled self-attested | Profile shows only published |
| AC-007.2 | Pending and declined never on the profile | Never appear |
| AC-007.3 | Explicit empty state. Owner sees a call to action | Empty profile |
| AC-007.4 | Unreachable PDS stated plainly | PDS unreachable |
| AC-007.5 | Confidence = stored / 10000 plus bucket | (WD-10) |
| AC-008.1 | Editable preview with the profile link before posting | Preview and confirm |
| AC-008.2 | Post only on explicit "Post to Bluesky". Never automatic | Nothing posted without consent |
| AC-008.3 | Text references only published, non-retracted claims | Never mentions unapproved |
| AC-008.4 | "Don't post" or leaving → no post, no state change | Nothing posted without consent |
| AC-008.5 | No share action with 0 published | Unavailable with nothing published |
| AC-008.6 | Failure explained with retry. Profile unaffected | Failed post |
| AC-009.1 | Peer pull accepts and stores provenance = self-attested | Maria pulls |
| AC-009.2 | Indexer and search include them, attributed, marked self-attested | Search shows |
| AC-009.3 | Viewer labels self-attested, never unverified | Maria pulls |
| AC-009.4 | App-signed verification and markers unchanged | App-signed unchanged |
| AC-009.5 | Integrity failures refused | Tampered refused |
| AC-010.1 | Re-verify before every scrape. Failure → no scrape, no suggestions, link unverified, actionable message | Re-checked; failed re-check |
| AC-010.2 | Pending hidden (not deleted) and not approvable while unverified | Failed re-check |
| AC-010.3 | **Published claims unchanged on verification failure** | Failed re-check |
| AC-010.4 | Re-verify restores scanning and un-hides pending | Re-verifying restores |
| AC-010.5 | No re-offer of published or declined keys. Summary counts shown | Only new suggestions |
| AC-011.1 | Retraction only after preview and confirm | Priya retracts |
| AC-011.2 | References the original CID. The original is never deleted | Never deletes |
| AC-011.3 | Retracted claims excluded from profile and share text | Priya retracts |
| AC-011.4 | Failure leaves the claim active, with retry | (failure mode) |
| AC-012.1 | Confirmation states what is deleted and what is kept | (dialog) |
| AC-012.2 | All app-side state for the DID purged. Session ends | Forget me removes |
| AC-012.3 | No PDS record modified or deleted | Published untouched |
| AC-012.4 | Cancel changes nothing | Cancel keeps everything |

## Accessibility (all pages)

- Every page has a descriptive title. All interactive elements are reachable by keyboard
  with a visible focus ring. Text contrast is at least 4.5:1. Every input has a label.
  Error messages name the field and the fix (WCAG 2.2 AA).
