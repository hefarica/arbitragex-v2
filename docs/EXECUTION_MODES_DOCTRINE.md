# EXECUTION MODES DOCTRINE — ArbitrageX v2

> **Source of truth for §34 of the operator doctrine (CLAUDE.md).** This
> document formalizes what the code actually enforces — it does not add new
> restrictions nor remove existing ones. Verified against
> `backend/relays-client/src/live_exec_policy.rs` (2026-09-24).

## 1. Doctrine

1. **Hot-path mode-invariant.** Discovery, 264 cartridges, 32 mathematical
   operators, routes, `SizeOptimizer`, simulation and risk/evidence gates are
   **identical** in all trading modes. The Master Matrix 264×32 is
   mode-invariant: all 8.448 strategy↔operator relations have the same role in
   `LIVE_MAINNET`, `TESTNET` and `PAPER_SHADOW`.
2. **`LIVE_MAINNET` is canonical.** Everything is designed and judged against:
   *"Would this work correctly with real capital on LIVE MAINNET?"*
3. **Modes differ ONLY at the execution terminus:**
   - `LIVE_MAINNET` → real capital → mainnet broadcast → real on-chain settlement.
   - `TESTNET` → testnet funds → testnet broadcast → on-chain settlement (not real).
   - `PAPER_SHADOW` → simulated capital → **NO broadcast** → simulated ledger.
4. **`OFF` / Kill-switch is NOT a trading mode** — it is an independent control
   state (stops everything regardless of mode).

## 2. Where the mode actually switches

The switch lives in `backend/relays-client/src/live_exec_policy.rs` — the
**ONLY binary that can sign and broadcast**. The mechanism (verified):

| Claim | Code evidence | Status |
|---|---|---|
| Allowlist per chain via env | `live_exec_policy.rs:21-22` reads `ARBX_LIVE_EXEC_ENABLED`/`ARBX_LIVE_EXEC_CHAINS` | ✅ |
| Default: Sepolia `11155111` | `:3` `DEFAULT_LIVE_CHAINS: &[u64] = &[11_155_111]` | ✅ |
| Mainnet (chain_id=1) IS supported | Test `:71-76`: `from_raw("true","1,11155111")` → `assert_broadcast_allowed(1).is_ok()` | ✅ |
| No env = default-deny total | `:37` requires `enabled == Some("true")`; test `:59-69` | ✅ |
| Malformed env never partially enables | `:30` `filter(>\|x\| *x > 0)` + `Option` collect → empty list → deny-all; test `:78-84` covers `"1,garbage"`, `"0,1"`, `"1,,"`, `"-1"` | ✅ |
| `MainnetRefused` never existed | Enum `:6-11` only has `NotEnabled` + `ChainNotAllowed` | ✅ |

**Flipping to `LIVE_MAINNET` with real capital = irreversible, gated action**
requiring: §32/§33 satisfaction, `arbx-*` skills PASS, and explicit operator
authorization (§34.5). No additional mainnet restriction beyond the env switch
may be added (operator order 2026-09-17).

## 3. Migration flags (TEMPORARY)

`ARBX_ORCHESTRATOR_MODE` (`v1`/`v2`/`shadow`/`off`, default V1 —
`scanner.rs:147-177`) and `ARBX_CARTRIDGE_MODE` (`off`/`shadow`/`active`,
default Off — `cartridge_boot.rs:63-66`) exist **only as migration flags**.
They do NOT define trading-mode semantics.

## 4. Cartridge couple/decouple (CARTRIDGE-CONTROL)

Per-cartridge enable/disable is orthogonal to trading mode: it controls whether
a cartridge is EVALUATED, never whether it broadcasts. See
`backend/searcher-rs/src/cartridge_control.rs` + `GET/PUT /api/v1/cartridges/control`.

## 5. Open questions

- The `§34.5` permanent conditional authorization references skill gates that
  must be verified with reproducible artifacts (not doc claims).
- Env vars `ARBX_LIVE_EXEC_*` are documented here but still missing from
  `.env.example` (DOC-05).
