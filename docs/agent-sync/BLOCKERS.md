# Blockers — verified, with in-flight PR mapping (2026-07-01)

> Source: dossier #248 (`docs/mainnet-readiness.md`), verified `file:line`. "In-flight PR" column checked
> live 2026-07-01 — no open PR touched the P0-file paths except as noted.

## P0 — must close before any mainnet flip
| ID | Blocker | Evidence | In-flight PR | Owner |
|---|---|---|---|---|
| P0-1 | No KMS/HSM signer — raw local `LocalWallet` | `signer.rs:18`; `KMS_KEY_ID=ABSENT_BY_POLICY` | none | operator + S3 |
| P0-2 | `security.yml` RED — audit allowlist expired 2026-06-30 | `gh run` failure @ 2026-07-01 (non-required check) | dependabot #181/#145/#175/#144/#220/#233 partially resolve | operator + S5 |
| P0-3 | no-hardcode gate neutered + 61 live violations | `lint-no-hardcode.sh:141` `exit 0`; script run → 61 | none | S5 |
| P0-4 | rollback path — `hardened-vps-rollback.yml` is a dangling reference | `hardened-vps-deploy.yml:727`; glob → no file | none | S5 + operator |
| P0-5 | `UPGRADER_ROLE` retained by deployer post-handoff → instant-drain | `DeployMainnet.s.sol:214-236` moves only DEFAULT_ADMIN_ROLE; `_authorizeUpgrade` gated on UPGRADER_ROLE (`ArbitrageExecutor.sol:649`) | none | **S4** (comment on #248) |

## P1 — must close before canary
| ID | Blocker | Evidence | Owner |
|---|---|---|---|
| P1-1 | gas-price ceiling not enforced at send | `bundle_builder.rs:216` no ceiling vs `max_gas_price_gwei` (adversarially CONFIRMED) | S3 |
| P1-2 | no aggregate exposure cap; `max_parallel_executions=8` is dead config | CONFIRMED — no running-sum/semaphore anywhere | S3 |
| P1-3 | capital-protective breakers (drawdown/gas-burn/latency) not wired | `risk_ledger.rs` 0 callers; pre-execute Check 12 reads a key with no writer | S3 |
| P1-4 | 5 canary metrics not emitted → dead alerts incl. `PaperModeDisabled` | `alerts.rules.yml` TODO; grep → 0 emitters | S2 |
| P1-5 | nonce self-DoS — `refresh()` dead code | `nonce_manager.rs:56` 0 callers; burns nonce on non-inclusion | S3 |

## P2 — hygiene before sign-off
| ID | Blocker | In-flight PR | Owner |
|---|---|---|---|
| P2-1 | prod VPS IP hardcoded in 18 public workflows | **#226 (in-flight)** | S5 |
| P2-2 | ADR-0005 stale; DR-drill K8s/AWS fiction; runbook drift | none | S5 |
| P2-3 | `DeployMultichain` handoff parity; `max_price_impact_pct` unit | none | S4 |
| P2-4 | ValidatedPlan carrier read verbatim from unauthenticated Redis | none | S3 |
| P2-5 | `U256::as_u128` panic in the value-cap guard | none | S3 |
