# Trust Policy — ArbitrageX v2

Every delivery, audit, status report, slide, and commit message must label claims
using the three-category taxonomy below. This protects credibility and keeps us
honest when infrastructure or external dependencies are incomplete.

## Three categories

| Category | When to use | How to write it |
|---|---|---|
| **Verified** | Material that was inspected/executed during the session (files, schemas, endpoints that returned real HTTP status, commands that ran). | "confirmed finding", "verified behavior", "tested locally". |
| **Narrative** | Claims from READMEs, specs, design docs, roadmap — not yet validated by an independent run. | "documented objective", "specified behavior", "claim from canonical doc". |
| **Unavailable** | Components, repos, endpoints or artifacts confirmed as out of session scope or not accessible. | "outside current scope", "no artifact available", "blocked by missing credentials". |

## Canonical rule on aggressive metrics

Performance claims — ROI, success rate, latency p95/p99, throughput, revert rate —
**are NEVER benchmarks** unless the session measured them independently. They are
either `Verified` (measured here) or `Narrative` (documented, not measured).

## Mapping to implementation states

| Trust label | Sprint status label (used in checklists) |
|---|---|
| Verified | `[OK]` |
| Narrative / tested-partially | `[PARCIAL]` |
| Narrative / untested | `[PENDIENTE]` |
| Unavailable | `[BLOQUEADO]` |

## Required disclosure

Every Sprint deliverable includes a section "Validaciones realmente ejecutadas" that
enumerates every command executed AND every command NOT executed (with reason).
If nothing was run, say so — never imply otherwise.

## Consequences of violating this policy

- Any PR that presents narrative claims as verified findings must be blocked in review.
- Any dashboard or docs that display synthesized data must be flagged as dev-only.
- Kill-switch + 501 responses exist so that "we have no real data" is visible as system
  state instead of hidden behind fake values.
