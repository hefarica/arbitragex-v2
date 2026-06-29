---
name: arbx-skills
description: 'Consolidated knowledge base and operating manual for ArbitrageX v2 (Rust hot-path + TypeScript control-plane + Cloudflare Workers + Next.js MEV/arbitrage platform on VPS <VPS_IP>, ssh alias arbx, repo hefarica/arbitragex-v2). Invoke at the start of any ArbitrageX session and whenever the operator types /arbx-skills, /arbx-skills gates, or /arbx-skills full. Encodes the security invariants, the 12 doctrinal gates, the paper-shadow operational truth (real blockers and what 0 paper trades means), the CI/CD and .env deploy pipelines, the credential-sourcing map, and the Windows/SSH/Excel/CI gotchas, so a fresh session avoids re-discovering facts, leaking secrets, breaking the live system, or flipping paper_mode.'
---

# ArbitrageX v2 — Consolidated Skill (arbx-skills)

This skill is the single index of everything known about operating ArbitrageX v2. It replaces
re-discovery: the architecture, the doctrine, the live-system truth, the deploy pipeline, the
credential sources, and the environment traps are all captured here and in the two reference files.

## Invocation modes (branch on the argument)

- **`/arbx-skills`** (no argument) → present the **overview** below + the always-on invariants, and
  load the session-truth quick-map. Do NOT dump the full references unless asked.
- **`/arbx-skills gates`** → read `references/gates.md` and apply the 12 doctrinal gates to the
  current work. Announce which gates fire.
- **`/arbx-skills full`** → read BOTH `references/gates.md` AND `references/operational-runbook.md`
  in full; load the complete operating knowledge before acting. Use this when onboarding a fresh
  session, planning a deploy, touching credentials, or debugging the live pipeline.

## INVARIANTS — always on, every mode, non-negotiable

1. **`paper_mode=true` is NEVER flipped to `false` by Claude.** Only the operator, manually, after
   review (it lives at `configs/app.toml:20`). Capital stays at 0. This is the master safety gate.
2. **NEVER print or leak a secret VALUE** to the chat transcript. Report key NAMES, lengths, presence,
   non-secret config flags (e.g. `anvil`, `active`, `shadow`), and URL hosts only. (On 2026-06-15 a
   masked workbook read leaked 4 creds → those must be rotated. Do not repeat.) When generating keys
   (`cast wallet new`), the private key goes value→cell/file directly, only the public address is shown.
3. **No-hardcode doctrine**: productive data (RPC URLs, addresses, keys, capital, thresholds) is never
   a literal in source — it is solicited and read from `.env`/typed config. See gate in references.
4. **No MEV predatorio** (sandwich, frontrun-against-user, oracle manipulation, JIT-displacement,
   time-bandit). Ethical arbitrage only.
5. **No mainnet broadcast / no destructive VPS action without explicit operator OK** (pre-execute gate).
6. **Fail-honest (R8)**: never fabricate data; surface 503/501 honestly; "0 paper trades" can be the
   correct truth, not a bug (see runbook).
7. **Verify against reality** before declaring anything "done/perfect" — read the live VPS/CI/DB, do
   not trust a workflow's "success" alone or report a phantom deploy.

## Overview (the 60-second map)

- **What it is**: paper-shadow MEV/arbitrage pipeline. Detection (searcher-rs) → price enrich
  (token-enricher → Redis) → scoring (advisory) → simulation (anvil fork via sim-ctl) → emit paper
  trade only when `rejection_reason=None`.
- **Where it lives**: VPS `<VPS_IP>` = ssh alias `arbx`, deploy path `/opt/arbitragex-v2`
  (git checkout of `main`). Repo `github.com/hefarica/arbitragex-v2`. Local working repo:
  `C:\Users\HFRC\Desktop\arbitragex_v2_productivo_full`.
- **Current truth** (keep updated): paper-shadow infra is healthy; the audit doc dated 2026-06-14 is
  mostly STALE. The real blockers and what's done/pending are in the runbook §"Paper-shadow truth".
- **Two deploy planes**: (a) CODE via CI/CD — `deploy-vps.yml` (manual `workflow_dispatch`, gated);
  (b) SECRETS via the Excel `.env Production` sheet → `RunFullSyncCycle` macro → VPS `.env`.

## 10-item delivery format (use for every substantial deliverable)

1. Objetivo · 2. Skills/gates aplicados · 3. Inputs productivos solicitados · 4. Inputs pendientes ·
5. Riesgos · 6. Validaciones hechas · 7. Reversibilidad · 8. Métricas de éxito · 9. Próximo paso ·
10. Archivos/referencias tocadas (con rangos de línea).

## Bundled references

- **`references/gates.md`** — the 12 doctrinal gates (mev-ethics, net-profit, simulation-mandatory,
  pre-execute, pre-edit-audit, no-hardcode, contract-atomicity, flash-loan, rpc-failover,
  risk-limits, token-safety, paper-trade-first), with triggers and what each enforces.
- **`references/operational-runbook.md`** — architecture & infra; the CI/CD + .env deploy pipelines
  (with the exact commands and the `restart`→`recreate` gotcha); the paper-shadow truth (decimals
  INT2/INT4 fix + migration 098, why 0 paper trades is correct, the rejection histogram); the
  credential-sourcing map (LOCAL vs PROVIDER, the 7 live-secrets, Crucible, Holesky→Sepolia,
  GoPlus=key+secret, Tenderly=3 vars+paid); and the Windows/SSH/Excel/PowerShell/CI gotchas.

When in doubt, prefer `/arbx-skills full` and read both references before acting on the live system.
