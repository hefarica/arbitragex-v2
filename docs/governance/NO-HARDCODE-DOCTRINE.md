# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# ArbitrageX v2 â€” No-Hardcode Doctrine (Immutable)

**Status:** Immutable. Overrides expedience. Applies to every module, every PR, every commit.
**Adopted:** 2026-04-22
**Enforced by:** spec/plan reviews, CI checks (to be added in Phase 0.5 below), code review.

---

## The rule

No productive operational value lives in code. "Productive" means anything the system would actually act on in a real environment:

- Credentials, secrets, API keys, signing keys, JWTs
- Real endpoints (RPC, relays, external APIs, webhooks)
- Wallet / signer addresses
- Contract addresses **where the operator has a choice** (routers, pools, feeds they opt in to)
- Asset catalogs, token allow-lists
- Risk thresholds, scoring weights, business-side caps
- Strategy enable-flags
- User identities, org IDs, chain enable-flags

If a system ever *acts* on a value, that value must come from one of the eight allowed sources below, **not from an `if`, `const`, or inline literal in a service binary**.

### Allowed sources (exhaustive)

1. Explicit user input (UI form, CLI prompt)
2. In-app configuration screen backed by the database
3. Environment variables (fail-fast if missing)
4. Secret manager / Vault (AppRole per service)
5. Database row the operator owns
6. Authenticated external service (OIDC, signed webhook, API key with rotation)
7. Validated editable config file (schema-enforced)
8. Guided onboarding flow that writes into one of the above

Anything outside this list is a violation.

### Honesty rule

If a real value is not yet available, the feature is marked in one of four states and **never faked**:

- `IMPLEMENTED` â€” verified end-to-end with real data
- `PENDING_CREDENTIALS` â€” code done, blocked on a secret/key
- `PENDING_CONFIG` â€” code done, blocked on operator choice in UI/DB
- `DESIGNED` â€” spec exists, code not written yet

### Distinction between *canonical protocol constants* and *operational config*

Canonical protocol constants (Uniswap V2/V3 router addresses on mainnet, Sushi router, the canonical WETH address) **may** live in code because they are immutable public facts tied to a deployed contract. They MUST:

- Be clearly labeled as "protocol catalog, not operator config"
- Be accompanied by a test asserting the byte value
- Be auditable (comment citing the source)
- Be swappable â€” the service MUST look them up via a catalog function, never by inline literal at the call site

This is not an escape hatch. A router the *operator chose to use* is operational; the *existence* of UniswapV2Router02 at `0x7a25â€¦` on chain 1 is a protocol fact.

### Permitted defaults

- Placeholder literals in `.env.example` that cannot be confused with real values (`REPLACE_ME_*`)
- Example-only test fixtures inside `*.test.ts` / `#[cfg(test)]` blocks
- Empty strings in config schemas when the field is mandatory for a feature (forces operator to complete before the feature activates)
- Dev-only flags gated behind `ENV=development` + fail-closed in staging/prod

### Forbidden defaults

- Endpoints that would work if left unchanged (`https://relay.flashbots.net` as a default in code)
- Tokens/passwords that let the system "appear to work" (`dev_admin_token_change_me`)
- Assumed contract/pool/token addresses beyond the canonical catalog
- Assumed risk thresholds or scoring weights in code (must come from DB/config with the operator's sign-off)

---

## Progressive solicitation â€” 5-phase model

The application never asks for everything at once. Each feature declares which phase its data belongs to. The operator is shown only what the current phase needs.

| Phase | What it unlocks | Data requested |
|-------|-----------------|----------------|
| **1. Install & boot** | Stack comes up, health endpoints green, no execution | DB passwords (generated), admin token (generated), edge token (generated), JWT secret (generated). All generated locally during `arbx init`; operator is shown, then they are persisted to Vault. |
| **2. Connect primary services** | Read-only observability + mempool detection | RPC HTTP + WS per enabled chain, Grafana admin password, Slack webhook (optional) |
| **3. Advanced features** | Simulation + selector scoring + token safety | Anvil fork URL (or reuse HTTP RPC), token-safety provider choice + API key (if external), relay list + per-relay auth, initial scoring weights sign-off |
| **4. Real testing** | Paper-mode execution against real relays, recon learning | Flashbots signer key (zero-balance), private-relay credentials, builder list, per-chain gas cap sign-off |
| **5. Production operation** | Remove paper-mode rail, allow real-capital execution | Funded signer key, insurance cap sign-off, PagerDuty integration key, backup off-site credentials, incident contacts |

**Gating rule:** a feature belonging to phase N cannot activate if its dependencies are missing. The UI shows the gap and the path to close it; it does not silently degrade or fabricate.

---

## Required delivery format (every major change)

Every spec/plan/PR that introduces or alters a feature MUST include the 10-item block below (can be an appendix):

1. No-hardcode rules applied â€” which rules from this doctrine were enforced.
2. Data requirements matrix â€” table of what's needed, where it comes from.
3. Progressive solicitation flow â€” phase + step number, UX description.
4. Sensitive vs non-sensitive inventory.
5. Validation mechanism per datum (schema, regex, network-probe, cryptographic).
6. Storage mechanism per datum (env, DB column with encryption flag, Vault path).
7. Features structurally ready but pending real data â€” the `PENDING_*` list.
8. No-hardcode checklist (see below) â€” copy-paste and check boxes.
9. Validations executed â€” grep results, tests run.
10. Open risks if data is missing â€” what breaks and how the UI communicates it.

### No-hardcode checklist (canonical)

- [ ] No credentials embedded
- [ ] No productive endpoints embedded (non-canonical)
- [ ] No contracts embedded without a config/catalog indirection
- [ ] No wallet/signer addresses embedded
- [ ] No API keys embedded
- [ ] No business thresholds embedded without config
- [ ] No productive asset lists embedded without a real source
- [ ] No productive risk parameters embedded
- [ ] Every external dependency asks for its datum at the correct step
- [ ] Every critical config is validated at boot (fail-fast)
- [ ] Every sensitive config lives outside code (env/Vault/DB)
- [ ] Every feature declares its data dependencies explicitly
- [ ] The app knows when it can't operate and says so in the UI
- [ ] The app never appears to operate without real data

---

## Enforcement

Phase 0.5 (to land in the next commit after this doctrine) adds:

- `automation/tools/lint-no-hardcode.sh` â€” grep-based CI check over `backend/`, `shared-ts/`, `edge/`, `frontend/`.
- CI job `.github/workflows/no-hardcode.yml` â€” fails on new productive literals (allow-list: canonical protocol catalog entries in `shared-rs/src/chains.rs`, test fixtures under `*.test.ts` / `#[cfg(test)]`, documentation).
- PR template: requires the 10-item block in the description.

---

## Author's note on this codebase (as of 2026-04-22)

The current repository has pre-doctrine violations documented in [`AUDIT-2026-04-22.md`](./AUDIT-2026-04-22.md). Remediation is tracked there. Every violation has one of three outcomes:

- **Fix now** â€” trivial extractions into env/config.
- **Fix during the owning phase** â€” e.g. removed/replaced when that phase's credentials flow is built.
- **Accept with audit note** â€” when the literal is a canonical protocol constant, kept in `chains.rs`-style catalog with tests.

No new violations may be introduced. Every PR that adds one is blocked.

