# OMEGA-8 / M5 Capa 4 Fase 14 (P2-CSP-1) — Content-Security-Policy Enforcement Plan

Status: **Report-Only enforced** in production today. Plan to flip to **enforce
mode** governed by the operator flag `ARBX_CSP_ENFORCE=true`.

## Goal

Move from a decorative `Content-Security-Policy-Report-Only` header to a real
`Content-Security-Policy` header that blocks unauthorised script/style/img/connect
sources at the browser level — without breaking the operator console.

## Pre-conditions to flip

1. **CSP report endpoint is live.** Add `report-uri` / `report-to` directives
   that POST violations to an api-server route (e.g. `/admin/csp-reports`) so
   we can audit what would be blocked before flipping.
2. **7 days of clean reports.** No `script-src` or `connect-src` violations in
   the last 7 days for the routes the operator actually uses.
3. **Stripe/PagerDuty/Sentry dynamic URLs whitelisted.** Document every
   third-party origin the page hits (today: edge, WSS, optionally Sentry
   ingest). Build a `connect-src` directive that covers all of them.
4. **Inline scripts audited.** Any remaining inline `<script>` must be hashed
   (`'sha256-...'`) or moved to an external file. Same for inline styles where
   needed (`'unsafe-inline'` is **not** acceptable for `script-src`).
5. **`nonce` strategy chosen.** If we cannot move every inline to external,
   Next.js supports per-request CSP nonces; the middleware must inject the
   nonce into the page response.

## Flag

Add an env var read at build / boot:

```
ARBX_CSP_ENFORCE=true   → emit Content-Security-Policy header
ARBX_CSP_ENFORCE=false  → emit Content-Security-Policy-Report-Only header
```

The toggle lives in `next.config.js` `headers()` together with the existing
HSTS / Permissions-Policy headers. **HSTS must only be enabled when TLS is
actually terminated in front of the edge** — that gating is the operator's
responsibility on the VPS reverse proxy, not the frontend build.

## Skeleton — `next.config.js`

```js
function cspHeaderName() {
  return process.env.ARBX_CSP_ENFORCE === "true"
    ? "Content-Security-Policy"
    : "Content-Security-Policy-Report-Only";
}

function cspValue() {
  const connect = [
    "'self'",
    process.env.NEXT_PUBLIC_EDGE_URL,
    process.env.NEXT_PUBLIC_WS_URL,
  ]
    .filter(Boolean)
    .join(" ");
  return [
    "default-src 'self'",
    `connect-src ${connect}`,
    "img-src 'self' data:",
    "style-src 'self' 'unsafe-inline'",
    "script-src 'self'",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "report-to csp-violations",
  ].join("; ");
}
```

## Verification commands (run on the VPS, NOT locally)

```bash
# 1. Flag off (default): Report-Only header present.
curl -sI https://<edge-host>/ | grep -i 'content-security-policy'
# expected: Content-Security-Policy-Report-Only: …

# 2. Flag on: enforcing header present.
ARBX_CSP_ENFORCE=true … docker compose up -d frontend
curl -sI https://<edge-host>/ | grep -i 'content-security-policy'
# expected: Content-Security-Policy: …
```

## Rollback

If a routine flip breaks the console, the operator unsets `ARBX_CSP_ENFORCE`
and rebuilds the frontend container (R3 cache-busting rule applies — variables
are baked at `next build` time).

## Out of scope for M5

- Implementing the `/admin/csp-reports` endpoint (backend ticket).
- Replacing every inline style with classes (low priority refactor).
- Subresource Integrity (SRI) for third-party scripts (none in scope today).

---

**Decision criterion:** 7 consecutive days of zero critical CSP violations in
the report endpoint → flip `ARBX_CSP_ENFORCE=true` in staging for 24h, then
production. Capture the timestamp in the operations runbook.
