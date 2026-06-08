# Bi-Eje Scoreboard — Quantitative Maturity Model

* Status: accepted
* Date: 2026-05-18
* Deciders: @hefarica
* Consulted: All OMEGA-100 roster
* Informed: All team members

## Context and Problem Statement

Platform maturity assessment is often subjective ("it looks good"). For a DeFi system managing capital, we need an objective, quantitative measure of readiness. The existing assessment was ad-hoc and could not answer: "Is this system ready for Paper Shadow?" or "What specifically is missing for Live?"

## Decision Drivers

* Need objective maturity metric
* Must separate runtime health from platform maturity
* Must be actionable (points to specific gaps)
* Must track progress over time

## Considered Options

* Option A: Subjective assessment — ad-hoc reviews
* Option B: Binary checklist — pass/fail per item
* Option C: Bi-Eje weighted model — Eje 1 (runtime 15%) + Eje 2 (maturity 85%)

## Decision Outcome

Chosen option: "Option C — Bi-Eje Model", because it separates immediate operational health from long-term platform maturity.

### Consequences

* Good, because quantitative and objective
* Good, because separates runtime from maturity concerns
* Good, because 14 phases provide clear roadmap
* Bad, because requires maintenance as platform evolves
* Bad, because score can be gamed by focusing on easy points

## Validation

Current score: 96.00/100 (Paper Shadow active)
- Eje 1: 14.50/15 (96.7%) — runtime health
- Eje 2: 81.50/85 (95.9%) — platform maturity

Hitós:
- Pre-Paper Shadow: ≥22% (achieved)
- Paper Shadow: ≥70% (achieved)
- Live: =100% (pending F-11 14d green)

## Pros and Cons of the Options

### Option A — Subjective

* Good, because fast assessment
* Bad, because not reproducible
* Bad, because no progress tracking
* Bad, because not actionable

### Option B — Binary Checklist

* Good, because objective
* Good, because actionable
* Bad, because all items weighted equally
* Bad, because no nuance

### Option C — Bi-Eje Weighted

* Good, because nuanced weighting
* Good, because clear roadmap via 14 phases
* Good, because separates runtime from maturity
* Bad, because complex to maintain
* Bad, because can be gamed

## More Information

* Fases: F-01 (Compilation) through F-14 (Performance Budget)
* Formula: TOTAL = Eje1(0-15) + Eje2(0-85)
* Related: [ADR-0001](0001-zero-mocks.md), [ADR-0002](0002-r8-fail-honest.md)
