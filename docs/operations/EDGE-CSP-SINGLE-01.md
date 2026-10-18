# EDGE-CSP-SINGLE-01 — one CSP owner per response (edge), and the connect-src decision

Status: decided 2026-10-18 · Owner: security (edge/app) · Gate: `G-EDGE-CSP-SINGLE-01`
(`scripts/gates/g-edge-csp-single-01.sh`)

## Symptom (measured)

`https://edge-arbx.ape-tv.net/` returned **two** `content-security-policy` headers:

1. `default-src 'none'; frame-ancestors 'none'` — the edge worker
   (`edge/worker/src/index.ts`, `secureHeaders()` applied to `app.use("*")`).
2. `default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; …;
   connect-src 'self' ws: wss: http://127.0.0.1:8080 http://127.0.0.1:8787;
   frame-ancestors 'none'; base-uri 'self'; form-action 'self';`
   — a **static nginx** policy served by the VPS (`Server: nginx/1.24.0 (Ubuntu)`),
   proven by probing the origin directly (no Cloudflare): `http://195.201.235.70/api/health`
   returns both headers, and `http://195.201.235.70:8787/health` (nginx `deny all`)
   returns the static one alone.

Apex `https://arbx.ape-tv.net/` (Cloudflare → nginx `/` → frontend:5173) returned a
single, clean **Report-Only** policy from Next — no localhost.

### Why "two headers" is a real defect

Two CSP headers on one response do not override — the browser enforces the
**intersection** (the most restrictive union). So the effective edge policy was
capped by a hand-maintained string that (a) disagreed with the app policy and
(b) shipped `http://127.0.0.1:8080` / `:8787` into production `connect-src`.

### Root cause

The canonical nginx site (`config/nginx/arbitragex.conf`, installed at
`/etc/nginx/sites-enabled/arbitragex`) declares **no** CSP. The static policy
observed live matches **no file in the repo and no revision in git history** — it
is a drifted nginx include on the VPS (the legacy "security headers" set, same
shape as `infra/nginx/nginx.conf` / `frontend/nginx.conf`). This is the hazard
already documented in `audits/omniscience-integration-2026-09-06/WO-09-DESIGN.md`
("dos CSP enforcing se intersectan") and it materialised once nginx entered the
edge path (SEC-EDGE-BYPASS-01). The live stack was brought up by a manual
`compose up` without the deploy workflow's exports (same cause as
`/status → deploy.sha: unknown`), so the canonical sync never ran.

## Decision

**The application layer owns the CSP; nginx emits none.** One header per response.

- **Edge / JSON API (worker):** `default-src 'none'; frame-ancestors 'none'`.
  The edge serves only JSON — a document with no subresources — so `default-src
  'none'` is both correct and minimal, and `frame-ancestors 'none'` blocks framing.
  It needs **no `connect-src`** at all: a `connect-src` only constrains fetches
  made *from* a document, and no HTML/JS document is served from the edge.
- **Browser HTML (Next):** `frontend/next.config.js` `headers()` owns the policy.
  Its `connect-src` must list exactly, and only:
  `'self' ws: wss: <NEXT_PUBLIC_EDGE_URL> https://api.web3modal.org https://pulse.walletconnect.org`
  — built from env at build time, **never a literal `localhost`/`127.0.0.1`**
  (the R2 guard in `next.config.js` already hard-fails a localhost prod build).
  Today that policy ships as `Content-Security-Policy-Report-Only`; the enforcing
  flip (`ARBX_CSP_ENFORCE=true`) is a separate, operator-gated change.
- **nginx (all layers):** never emits `Content-Security-Policy`. Static nginx
  headers (X-Frame-Options, nosniff, HSTS, Referrer-Policy, Permissions-Policy)
  are fine and unaffected.

Rationale: keeps a single source of truth per response, removes the drift surface
(nginx strings can't be built from env, so they rot), and makes the edge policy
provably localhost-free.

## Enforcement

- Repo layer: `scripts/gates/g-edge-csp-single-01.sh` (asserts no `add_header
  Content-Security-Policy` in tracked `*.conf`, plus the live checks below).
- Deploy layer: `scripts/vps/verify-deploy.sh` §8d greps the live nginx config
  surface (`/etc/nginx/nginx.conf`, `/etc/nginx/conf.d/*.conf`,
  `/etc/nginx/sites-enabled/*`) for a CSP `add_header`.
- Gate `G-EDGE-CSP-SINGLE-01`:
  `curl -sD - https://edge-arbx.ape-tv.net/ | grep -ci content-security-policy` == 1,
  and 0 occurrences of `127.0.0.1`.

## Fix (two layers)

1. **Repo** — remove the CSP `add_header` from `infra/nginx/nginx.conf` and
   `frontend/nginx.conf`; document the contract in `config/nginx/arbitragex.conf`;
   add the gate + this note.
2. **Deploy sync** — remove the stale `add_header Content-Security-Policy` from
   the VPS nginx (`/etc/nginx/nginx.conf` or `/etc/nginx/conf.d/*.conf`), reinstall
   the canonical site (`scripts/vps/install-nginx-canonical.sh`), `nginx -t`,
   reload; then re-run the gate. Redeploy via the workflow (not a bare `compose up`)
   so `deploy.sha` is populated.
