# Zero-Mocks — All Components Consume Live Data Only

* Status: accepted
* Date: 2026-05-18
* Deciders: @hefarica
* Consulted: K-CORE, Z-SCHEMA
* Informed: All team members

## Context and Problem Statement

In traditional development, mocks and stubs are used to isolate components during testing. However, in a DeFi MEV arbitrage system, mocks hide real failures that only manifest in production: RPC timeouts, nonce conflicts, gas estimation errors, and block reorgs. The platform experienced multiple incidents where "all tests passed" but the system failed in production because mocked responses did not match real blockchain behavior.

## Decision Drivers

* Production fidelity must be guaranteed at all stages
* Blockchains are non-deterministic — mocks cannot replicate this
* Gas costs and timing vary per block
* Failures must be caught early, not hidden

## Considered Options

* Option A: Mock everything (traditional approach) — fast tests but hidden failures
* Option B: Hybrid (mocks for unit, live for integration) — moderate fidelity but gaps remain
* Option C: Live data only (Zero-Mocks) — slower tests but production fidelity

## Decision Outcome

Chosen option: "Option C — Live data only", because only live data captures the full complexity of blockchain interactions.

### Consequences

* Good, because all failures are caught before production
* Good, because tests validate real integration paths
* Good, because operators develop against real data from day one
* Bad, because tests are slower than mocked alternatives
* Bad, because requires live RPC endpoints for CI
* Bad, because paper mode (no real capital) is required for safe live testing

## Validation

Validated through:
- CI pipeline runs against live testnet RPCs
- Paper mode ensures no real capital at risk
- Ghost Protocol provides safe simulation environment

## Pros and Cons of the Options

### Option A — Mock Everything

* Good, because tests run fast (< 30 seconds)
* Good, because no external dependencies needed
* Bad, because hides real blockchain failures
* Bad, because creates false confidence
* Bad, because maintenance burden for mocks

### Option B — Hybrid

* Good, because balance between speed and fidelity
* Neutral, because moderate maintenance burden
* Bad, because gaps in integration coverage
* Bad, because boundary between mock and live is fragile

### Option C — Live Data Only

* Good, because production fidelity guaranteed
* Good, because no mock maintenance burden
* Bad, because slower CI (~5 minutes)
* Bad, because requires live RPC endpoints

## More Information

* Related: [ADR-0002](0002-r8-fail-honest.md) — R8 Fail-Honest
* Related: [ADR-0003](0003-bi-eje-scoreboard.md) — Bi-Eje Scoreboard
