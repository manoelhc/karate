# Research checkpoint

Status: partial. Recovered evidence inventory from the previous session on
2026-11-04; never equate a fetched page with a read page. The original PRD/business
documents predate this ledger and their unreviewed references remain provisional.

Boundary: Aikido AI Pentest workflows and directly relevant account, permission,
wallet, remediation and connector documentation. Unrelated SCA, firewall,
endpoint-security and general cloud-posture products are excluded at collection
level, not counted as read. English public docs are the selected source variant;
Karate en/pt-BR is a proposed market choice, not observed reference localization.

[Page ledger](research-pages.json) preserves PAGE-001…051 URLs, original capture
hashes, actual reading status, sections and media counts. Raw vendor captures stay
outside the repository. PAGE-052…055 record newly inspected public entry pages.
Their fetched excerpts do not establish full-page reading.

| Family | Canonical inventoried | Read | Discovered, not fully read | Inaccessible |
| --- | ---: | ---: | ---: | ---: |
| Documentation/support | 49 | 8 | 41 | 0 |
| Commercial (pricing and wallet) | 2 | 1 | 1 | 0 |
| Product entry page | 1 | 0 | 1 | 0 |
| Alternatives (three entry pages) | 3 | 0 | 3 | 0 |
| **Total** | **55** | **9** | **46** | **0** |

Reading coverage is **9/55 = 16.36%** of the current inventory. Discovery has not
reached a fixed point. No enumerated canonical page has been excluded, and no
duplicate alias increases this denominator. This is not a completeness claim.
Trust/status/changelog, independent customer evidence, authenticated workflows,
commercial terms and nested integration guides still need systematic inventory.
Login/entitlement access has not been attempted; do not label pages inaccessible
without observing that boundary. There are 93 uninspected image/frame references
in the recovered docs, plus uncounted media in the four public entry pages.
The unresolved link frontier has not been exhaustively enumerated (count unknown).

## Supported decisions

| Claim | Evidence / kind | Product consequence |
| --- | --- | --- |
| CLAIM-001 | PAGE-004/011: vendor documentation describes proof of domain control and explicit scope. | REQ-002 requires verified target scope; implementation of the verification adapter is still pending. |
| CLAIM-002 | PAGE-010/024: vendor describes reach-only dependencies, pause/cancel and request limits. | Never infer attack permission from reachability. Scope/lifecycle rules implemented; actual enforcement during network dispatch pending. |
| CLAIM-003 | PAGE-001: vendor describes separate discovery, exploitation, validation and reporting. | No model claim or missing header is a verified exploit. |
| CLAIM-004 | PAGE-033: coverage uses tested/discovered endpoints and has limitations. | Unknown denominator stays unknown. Code coverage is a separate engineering metric. |
| CLAIM-005 | PAGE-044: vendor groups reusable assessment configuration under named projects. | REQ-001 and tenant-scoped project persistence. |
| CLAIM-006 | PAGE-040: dated vendor pricing, USD 1/credit, 4,000-credit standard test, 10 credits/continuous agent. | Preserve assumptions in business plan; no purchase or price guarantee. |

All six are vendor-claimed behavior, not authenticated observations or independent
performance results. The scope policy and PostgreSQL tests in Karate are original
implementation work, not evidence that vendor internals work the same way.

## Contradictions and gaps

- FIND-001: source ledger absent at resume despite PRD citations. Fixed by recovery;
  substantive-reading gap stays open in TASK-001.
- FIND-008: the reference describes manual pause only during attacks; Karate's
  pause during preflight/discovery is explicitly proposed, not parity.
- FIND-009: the current marketing page's Mobile section advertises both Android
  and iOS, while the earlier PRD names Android. Broader mobile parity remains in
  scope; iOS details and documentation frontier require follow-up before coding.
- Competitor homepage snippets corroborate positioning, not throughput, accuracy,
  or economic performance. The earlier business plan's numerical assumptions are
  hypotheses. The Reddit anecdote has not been re-read in this continuation.

## Journey evidence

JOURNEY-001 is documentation-only: project → scope/ownership → identities and
context → safety settings → launch → coverage/findings → retest/report.
No UI journey, role/plan variant, error recovery, purchase, invite or assessment
was executed against the reference or competitors. Success/failure/locale/plan
coverage is therefore unobserved. Only isolated owned code/database fixtures ran.

Next research action: read PAGE-002/003/005–009 and remaining authentication guides,
refresh the documentation index, enumerate mobile/iOS links and sample reports,
and update canonical counts and requirement provenance without replacing gaps
with inferred implementation claims.
