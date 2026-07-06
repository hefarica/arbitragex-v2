# R8 Fail-Honest — Transparent System State Reporting

* Status: accepted
* Date: 2026-05-18
* Deciders: @hefarica
* Consulted: O-PTEL, S-VAULT
* Informed: All team members

## Context and Problem Statement

In distributed systems, it is common practice to hide degraded or failing services from end users through synthesis (showing cached data) or optimistic UI patterns. However, for a DeFi arbitrage platform where operators make capital allocation decisions based on system state, hiding degraded services is dangerous. An operator might increase position size while the risk engine is offline, or believe a strategy is healthy when its underlying RPC is down.

## Decision Drivers

* Operators must see the true system state at all times
* Synthesized data can lead to bad capital decisions
* Degraded services must be visible, not hidden
* The System Guard Banner is the single source of truth

## Considered Options

* Option A: Optimistic UI — hide problems, show cached data
* Option B: Degraded-aware UI — show problems with graceful degradation
* Option C: Fail-Honest — show raw state, no synthesis, no hiding

## Decision Outcome

Chosen option: "Option C — Fail-Honest", because only raw state enables correct operator decisions.

### Consequences

* Good, because operators see the exact system state
* Good, because no hidden failures
* Good, because degraded services are immediately actionable
* Bad, because UI can show many warnings during partial outages
* Bad, because requires robust operators trained on the system

## Validation

Validated through:
- System Guard Banner shows all 21 containers with exact status (healthy/degraded)
- Kill-switch state is always visible
- Paper mode indicator is always prominent
- No synthesis of "missing" data — gaps are shown

## Pros and Cons of the Options

### Option A — Optimistic UI

* Good, because cleaner user experience during issues
* Bad, because hides critical failures from operators
* Bad, because creates false confidence
* Bad, because delays incident response

### Option B — Degraded-aware UI

* Good, because shows problems with graceful handling
* Neutral, because moderate complexity
* Bad, because still synthesizes some data
* Bad, because operator must learn what is synthesized

### Option C — Fail-Honest

* Good, because full transparency
* Good, because fastest incident detection
* Bad, because more complex UI with many status indicators
* Bad, because requires operator training

## More Information

* Related: [ADR-0001](0001-zero-mocks.md) — Zero-Mocks
* Related: [ADR-0003](0003-bi-eje-scoreboard.md) — Bi-Eje Scoreboard
