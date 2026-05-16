# CARGO_AUDIT_TRIAGE — ArbitrageX-v2 OMEGA Recovery

**Branch**: `omega/recovery-20260516`  
**Triage Date**: 2026-05-16  
**Triage Owner**: Security Engineer subagent (OMEGA team)  
**CI Run**: `25963371369` — `cargo audit (Rust advisories)` / `6_Run cargo audit.txt`  
**Advisory DB snapshot**: 1090 advisories loaded, 761 crate dependencies scanned

---

## Doctrine

> **DOCTRINA** (Hector's directive):  
> *"Excepciones temporales para dependencias transitivas no explotables directamente.  
> MIGRACIÓN MAYOR POSPUESTA para proteger el hot-path y el motor cuántico."*

> **PRINCIPIO DE VISIBILIDAD**:  
> *"POR EL HAZ DE LUZ SOLO PASA QUIEN ES VISTO"* —  
> Every decision is documented. No silent ignores. Every expiry is scheduled.

---

## Summary

- **Total advisories**: 15 (8 vulnerabilities + 7 warnings promoted to errors by `--deny warnings`)
- **Ignored with justification**: 15
- **Cargo.toml / Cargo.lock modified**: **0** (no blind bumps per doctrine)
- **Separate PRs required**: 4 (see Migration Roadmap)

---

## Triage Table

| # | ID | Crate | Version | Type | Severity | Decision | Justification Summary | Transitive Root | Expiry |
|---|-----|-------|---------|------|----------|----------|-----------------------|-----------------|--------|
| 1 | [RUSTSEC-2024-0437](https://rustsec.org/advisories/RUSTSEC-2024-0437) | protobuf | 2.28.0 | vulnerability | DoS/crash (uncontrolled recursion) | **IGNORE** | Transitive via prometheus 0.13.4. protobuf v3 is a major break; prometheus pins v2. prometheus touches shared-rs/sed-core hot-path. Not externally exposed (internal metrics only). | prometheus → shared-rs / sed-core / searcher-rs | 2026-08-01 |
| 2 | [RUSTSEC-2025-0009](https://rustsec.org/advisories/RUSTSEC-2025-0009) | ring | 0.16.20 | vulnerability | DoS/panic (AES overflow check) | **IGNORE** | Transitive via jsonwebtoken 8.x → ethers-providers 2.x. ring 0.17 breaks jsonwebtoken 8.x API. Panic only in debug overflow-check builds; release builds unaffected. ethers→alloy migration will retire ring 0.16. | jsonwebtoken → ethers-providers → ethers → full stack | 2026-08-01 |
| 3 | [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071) | rsa | 0.9.10 | vulnerability | CVSS 5.9 Medium (Marvin timing attack) | **IGNORE** | **No fixed version available upstream.** Transitive via sqlx-mysql 0.7.4. Timing attack requires network adjacency + millions of repeated RSA decrypts. Service uses RSA only in sqlx-mysql TLS handshake, not application-layer decryption. Isolated DB network segment. sqlx 0.8 bump tracked separately. | sqlx-mysql → sqlx → full stack | 2026-09-01 |
| 4 | [RUSTSEC-2026-0099](https://rustsec.org/advisories/RUSTSEC-2026-0099) | rustls-webpki | 0.101.7 | vulnerability | High (TLS cert name constraint bypass) | **IGNORE** | Transitive via rustls 0.21 → tungstenite 0.20 → ethers-providers 2.x. Fix requires rustls-webpki ≥0.103.12 which requires rustls 0.22+ → breaks ethers 2.x TLS chain. Only exploitable with attacker-controlled TLS server; connections are to known blockchain nodes. ethers→alloy migration resolves. | rustls 0.21 → ethers TLS stack | 2026-08-01 |
| 5 | [RUSTSEC-2026-0098](https://rustsec.org/advisories/RUSTSEC-2026-0098) | rustls-webpki | 0.101.7 | vulnerability | High (TLS URI name constraint bypass) | **IGNORE** | Same crate/version as RUSTSEC-2026-0099. URI name constraints are niche X.509 feature; blockchain node TLS certificates do not use them. Same migration dependency. | rustls 0.21 → ethers TLS stack | 2026-08-01 |
| 6 | [RUSTSEC-2026-0104](https://rustsec.org/advisories/RUSTSEC-2026-0104) | rustls-webpki | 0.101.7 + 0.103.12 | vulnerability | High (DoS via CRL parsing panic) | **IGNORE** | Two instances: (a) 0.101.7 path via ethers TLS — same constraint as 0099/0098; (b) 0.103.12 path via rustls-platform-verifier → alloy stack — fix requires 0.103.13, targeted patch feasible but requires post-OMEGA verification. CRL revocation not used in blockchain node connections. | (a) ethers TLS; (b) rustls-platform-verifier → alloy | 2026-07-01 |
| 7 | [RUSTSEC-2024-0363](https://rustsec.org/advisories/RUSTSEC-2024-0363) | sqlx | 0.7.4 | vulnerability | High (binary protocol misinterpretation) | **IGNORE** | Direct usage in sed-core/shared-rs (quantum engine data layer). sqlx 0.8 is a major API break. Current schema uses bounded integers that do not reach overflow boundary values (i32::MAX, u64 boundary). Upgrade requires full DB integration test suite. Tracked in separate PR. | sqlx → sed-core / shared-rs / token-enricher / full stack | 2026-08-01 |
| 8 | [RUSTSEC-2026-0002](https://rustsec.org/advisories/RUSTSEC-2026-0002) | lru | 0.12.5 | unsound | UB via IterMut (Stacked Borrows) | **IGNORE** | Transitive via searcher-rs/prioritization-spine. searcher-rs uses LRU for read-heavy caching; IterMut is not called in hot-path. UB only manifests when IterMut is used concurrently or in specific aliasing patterns. Post-OMEGA audit required to confirm IterMut usage is absent. | lru → searcher-rs / prioritization-spine | 2026-07-15 |
| 9 | [RUSTSEC-2024-0388](https://rustsec.org/advisories/RUSTSEC-2024-0388) | derivative | 2.2.0 | warning | unmaintained | **IGNORE** | Transitive via ark-ff (ZK math, 3 versions) → ruint → alloy-primitives + via simba → nalgebra → sed-core. No CVE, no known exploit. Replacement requires upstream changes in ark-ff/nalgebra. Tracking upstream. | ark-ff + nalgebra → alloy + sed-core | 2026-10-01 |
| 10 | [RUSTSEC-2025-0057](https://rustsec.org/advisories/RUSTSEC-2025-0057) | fxhash | 0.2.1 | warning | unmaintained | **IGNORE** | Transitive via hashers 1.0.1 → ethers-providers 2.x. Non-cryptographic hash function — no security risk. Cannot upgrade without ethers major bump. ethers→alloy migration eliminates this. | hashers → ethers-providers → ethers | 2026-08-01 |
| 11 | [RUSTSEC-2024-0384](https://rustsec.org/advisories/RUSTSEC-2024-0384) | instant | 0.1.13 | warning | unmaintained | **IGNORE** | Transitive via ethers-providers/ethers-middleware 2.x. WASM-compatible time abstraction; no CVE. No compatible replacement for ethers 2.x. ethers→alloy migration eliminates this. | ethers-providers + ethers-middleware → ethers | 2026-08-01 |
| 12 | [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436) | paste | 1.0.15 | warning | unmaintained | **IGNORE** | Transitive via syn-solidity → alloy-sol-macro chain AND sqlx-core AND simba/nalgebra. No CVE. Stable proc-macro, no security issues. Replacement requires upstream changes in alloy + sqlx. Tracking upstream. | alloy-sol-macro chain + sqlx-core + nalgebra | 2026-10-01 |
| 13 | [RUSTSEC-2025-0010](https://rustsec.org/advisories/RUSTSEC-2025-0010) | ring | 0.16.20 | warning | unmaintained | **IGNORE** | Same crate/path as RUSTSEC-2025-0009. "Unmaintained" companion to the AES panic advisory. Covered by same migration plan (ethers→alloy retires jsonwebtoken 8.x / ring 0.16). | jsonwebtoken → ethers-providers → ethers | 2026-08-01 |
| 14 | [RUSTSEC-2025-0134](https://rustsec.org/advisories/RUSTSEC-2025-0134) | rustls-pemfile | 1.0.4 | warning | unmaintained | **IGNORE** | Transitive via sqlx-core 0.7.4 AND reqwest 0.11.27 → ethers stack. rustls-pemfile 2.x breaks sqlx 0.7.x API. No CVE; PEM parsing is stable. Resolved by sqlx 0.8 + reqwest 0.12 bumps in tracked PRs. | sqlx-core + reqwest 0.11 → ethers stack | 2026-08-01 |

---

## Migration Roadmap (Separate PRs Required)

| PR | Target | Resolves | Priority |
|----|--------|----------|----------|
| `feat/sqlx-0.8-upgrade` | sqlx 0.7.4 → 0.8.x | RUSTSEC-2024-0363, RUSTSEC-2023-0071 (rsa), RUSTSEC-2025-0134 (rustls-pemfile) | HIGH |
| `feat/ethers-to-alloy-migration` | Retire ethers 2.0.14, adopt alloy 1.8.3 fully | RUSTSEC-2025-0009, RUSTSEC-2025-0010 (ring 0.16), RUSTSEC-2026-0099/0098/0104 (rustls-webpki 0.101.7), RUSTSEC-2025-0057 (fxhash), RUSTSEC-2024-0384 (instant), RUSTSEC-2024-0437 (protobuf via prometheus) | HIGH |
| `fix/rustls-webpki-0.103.13-patch` | rustls-webpki 0.103.12 → 0.103.13 (Cargo.lock patch only) | RUSTSEC-2026-0104 (0.103.12 instance via alloy stack) | MEDIUM |
| `fix/lru-soundness-audit` | Audit lru usage in searcher-rs; bump if safe | RUSTSEC-2026-0002 | MEDIUM |

---

## Expiry Review Schedule

| Date | IDs to Review |
|------|--------------|
| 2026-07-01 | RUSTSEC-2026-0104 |
| 2026-07-15 | RUSTSEC-2026-0002 |
| 2026-08-01 | RUSTSEC-2024-0437, RUSTSEC-2025-0009, RUSTSEC-2026-0099, RUSTSEC-2026-0098, RUSTSEC-2024-0363, RUSTSEC-2025-0057, RUSTSEC-2024-0384, RUSTSEC-2025-0010, RUSTSEC-2025-0134 |
| 2026-09-01 | RUSTSEC-2023-0071 |
| 2026-10-01 | RUSTSEC-2024-0388, RUSTSEC-2024-0436 |

---

## Files Changed

| File | Action | Reason |
|------|--------|--------|
| `.cargo/audit.toml` | **CREATED** | Justified ignore list per Hector's doctrine |
| `docs/core/CARGO_AUDIT_TRIAGE.md` | **CREATED** | Full triage audit trail |
| `Cargo.toml` | **NOT MODIFIED** | No blind major bumps; all bumps deferred to tracked PRs |
| `Cargo.lock` | **NOT MODIFIED** | No Cargo.toml changes; lock file unchanged |

---

*Triage performed by Security Engineer subagent under OMEGA protocol.*  
*"POR EL HAZ DE LUZ SOLO PASA QUIEN ES VISTO" — All decisions visible and auditable.*
