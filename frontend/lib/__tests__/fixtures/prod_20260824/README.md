# FE-0045: historical recording provenance

These files are the archived JSON payloads identified by `manifest.json`, not
fresh observations of production. The manifest timestamp is preserved as
recorded; it is not an assertion about the time of a new test run.

## Representation and integrity

The files are pretty-printed. The manifest's `bytes` and `sha256` refer to the
compact JSON wire representation. The test reconstructs that representation
with `JSON.stringify(JSON.parse(file))` and checks both fields. It never
regenerates the recorded hash to accept a changed value.

## Recovery of readiness_decision.json

Commit `0622c9e06d85e5df861874f3964e53779d35b906` (PR #477) added `go_a4: true`
to this old fixture while adding the field to the current API contract. That
changed the compact payload from 656 to 669 bytes without changing the original
manifest, so the file no longer represented the original recording.

The restored file is the exact Git blob
`e8f4d6c5f1f8de55b431675e809fe2ead3ba418b`, read from the parent of #477,
`3cde75daabea29b10b68435ae758976d0bed0555`. Its reconstructed wire representation
matches the existing 656-byte length and SHA-256
`80ee63a3e7c76a78e97657bb2651531e63b378e9e6c0db8d830206ab19efd271`.
No historical manifest, timestamp, verdict, or other payload was rewritten.

## Historical versus current contract

The current `ReadinessDecisionResponseSchema` still REQUIRES `go_a4`. The
historical test now proves that the original pre-field payload is rejected for
exactly that missing field and for no other reason. This is an explicit,
versioned incompatibility, not permission for current responses to omit it.
All other schema-bearing recordings continue to pass through the current
schemas. The documented absence of a paper/history mirror is unchanged.

These tests cannot detect response timeouts in the live domain. A.8 scoring and
A.6 circuit-breaker reads require separate public-browser verification against
the deployed SHA, their real current schemas, and the client's 5000ms budget.
Never alter a historical fixture to make a current contract or deployment look
successful.
