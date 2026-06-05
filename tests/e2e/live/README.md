# ArbitrageX v2 / QuantumX — LIVE functional suite

Playwright specs that exercise an **already-running** deployment (default: the
Cloudflare-fronted edge that serves the operator console SPA). Unlike the
sibling `../*.spec.ts` suite — which assumes a local `docker compose` stack —
these run against a live target and are **read-only by default** (doctrine
§32/§33: audit / shadow / read-only).

## Specs

| Spec | Gate | What it proves |
|------|------|----------------|
| `functional-live.spec.ts` | always | Every one of the 44 operator-console routes: hard-nav returns the SPA shell (not a shadowing edge route), mounts an `<h1>`, no "edge unreachable/error" banner, no RULE-00 forbidden tokens. Writes a coverage matrix to `audits/live/functional-live.json`. |
| `admin-gate-live.spec.ts` | always | Admin/mutating endpoints **reject** unauthenticated callers (401/403/404/405) — proves there is no open control surface. Never mutates. |
| `honest-state-live.spec.ts` | always | `/api/opportunities/live` is well-formed and `count === items.length` (no fabricated/hidden rows); the system-guard banner reflects a safe non-live posture; the opportunities page agrees with the edge (no synthesized rows). |
| `admin-mutating-live.spec.ts` | `ARBX_ADMIN_TOKEN` set | Self-reverting kill-switch round-trip (arm → `/status` reflects → disarm → reflects) + reasonless-arm guard. **Skipped** without a real token. Leaves the platform in the posture it found. |

## Run

```bash
cd tests/e2e
npm install                 # installs @playwright/test (browser is cached)

# read-only (safe — default target is the live edge)
npm run test:live

# everything incl. mutating (needs the real admin token; PowerShell):
$env:ARBX_ADMIN_TOKEN="<token>"; npm run test:live:all
```

### Target override

```bash
ARBX_FRONTEND_URL=https://edge-arbx.ape-tv.net   # default (CF public)
ARBX_FRONTEND_URL=http://195.201.235.70:8787     # raw VPS edge
ARBX_FRONTEND_URL=http://localhost:5173          # local next dev
```

## Coverage matrix verdicts

`audits/live/functional-live.json` (+ per-route files in `audits/live/routes/`):

- **PASS** — SPA shell served, `<h1>` mounted, interactive surface present.
- **EMPTY_OK** — rendered, but no buttons/forms/tables (honest empty page).
- **NOT_FOUND** — client 404 (route exists in repo but not on the live build).
- **SHADOWED** — an edge-worker route intercepted the hard-nav and returned
  JSON/text; deep-link / refresh of this page is **broken**. (Known: `/status`,
  shadowed by `edge/worker/src/index.ts` `app.get("/status", …)`.)
- **RATE_LIMITED** — the shared edge returned 429 during the sweep (a
  self-inflicted artifact of 44 rapid hard-navs, not a page defect). Retried
  once with backoff before classifying.
- **FAIL** — a real defect: ≥400 status, error banner, forbidden token, or no
  `<h1>` on a non-shadowed/non-404 page.

## Doctrine

- Read-only by default. Nothing here arms, signs, broadcasts, or flips the paper
  feed to live. The mutating spec is token-gated and fully self-reverting.
- Honest states (empty tables, "no opportunities") are PASS/EMPTY_OK, never FAIL.
- A `trading-config` PUT that activates the paper feed is **intentionally
  excluded** — that is an operator-gated decision, not an automated test action.
