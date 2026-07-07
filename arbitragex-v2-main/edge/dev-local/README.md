# Edge dev-local

Dev-only proxy mirroring the Cloudflare Worker's public interface for
developers running the full stack locally without a CF account.

**NOT for production** — production uses `edge/worker/` (Cloudflare Worker)
which provides WAF, DDoS protection, and rate-limiting that this shim lacks.

## WebSocket upgrade auth (audit N3, 2026-05-10)

The WebSocket upgrade path validates `X-ArbX-Admin-Token` (or
`sec-websocket-protocol`, or `?token=` query param) before proxying to
api-server. **Defense-in-depth**: even though api-server's `io.use()` also
authenticates the handshake (audit A1), filtering at the edge prevents
trust-on-first-hop fragility if this process is ever exposed beyond loopback.

Token sources, in priority order (mirrors `backend/api-server/src/websocket.ts`):

1. `X-ArbX-Admin-Token` header — tooling / `curl --header`
2. `sec-websocket-protocol` — browser fallback (`io.connect` cannot set custom
   headers, so the browser SDK is configured to send the token here)
3. `?token=` query param — last-resort browser fallback

Comparison uses `safeTokenEqual` (constant-time, length-safe) re-exported
from `@arbx/shared` to match the api-server implementation.

If `ARBX_ADMIN_TOKEN` is **unset** (pure dev / loopback), the proxy logs a
loud warning (`event: "ws.upgrade.unauthenticated"`) and allows the upgrade
through — explicit acknowledgment that dev mode trusts loopback. Production
must always have `ARBX_ADMIN_TOKEN` set; the boot validator
(`assertSecureBootTokens`) enforces this in api-server.

## Trust-on-first-hop semantics

This shim is intentionally weaker than the production Cloudflare Worker:

- No WAF.
- No DDoS protection.
- No global rate-limit (only the naive in-memory per-IP limiter in this file).
- No TLS termination — bind to loopback only or run behind a TLS proxy.

**Operational rule**: bind `EDGE_PORT` to `127.0.0.1` (or a Docker bridge
network) only. Never expose this service to a public network. If you need a
public-facing dev edge, use the production CF Worker pointed at a dev
api-server instead.
