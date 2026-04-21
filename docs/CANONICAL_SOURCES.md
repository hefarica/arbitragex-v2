# Canonical Sources — ArbitrageX v2

Based on the official toolkit (`Toolkit_ArbitrageX_Supreme_v3.0.pdf/.pptx`, by Manus AI),
each architectural layer has a canonical reference. When source repositories conflict,
the canonical source of the affected layer wins.

| Layer | Canonical repo | Function | Trust |
|---|---|---|---|
| Backend | `ARBITRAGEX-CONTABO-BACKEND` | Rust engine (searcher, selector, sim, relays, recon), Geth, Redis, PostgreSQL, Anvil, monitoring, deploy scripts. | Narrative (toolkit docs); current local repo mirrors its structure. |
| Edge | `ARBITRAGEXSUPREME` | Cloudflare Workers + KV + D1 separation between backend and external consumers. | Narrative — drives our `edge/worker/` wrangler design. |
| Frontend | `show-my-github-gems` | Dashboards, alerts, analytics. Explicitly described as "integración backend parcial y componentes mock" — we do NOT inherit those mocks. | Narrative — used only for layout ideas. |
| Automation | `ARBITRAGEXPLUS2025` | Scripts, installers, checklists, Windows/Excel utilities, multi-language tooling patterns. | Narrative. |
| UI variants | `lite-mult`, `multi-dex`, `alpha-flux`, `pro-control` | Visual / product patterns only. Not an architectural source. | Narrative — reference only. |

## Operating rules

- A task touching Rust engine, Geth, Redis, PostgreSQL, Anvil, relays, or monitoring
  starts from the **Backend canonical** repo's approach, not a UI variant.
- A task touching workers, wrangler, KV, D1, or Cloudflare-specific concerns starts
  from the **Edge canonical** repo.
- A task that only touches visualization goes to the **Frontend canonical** repo BUT
  MUST verify that any depicted metric actually exists in our `arbx_*` metrics
  (see `shared-rs/src/metrics.rs` and `shared-ts/src/metrics/index.ts`).
- "Look alike ≠ works alike". We never assume a dashboard's data path is wired just
  because a screenshot exists.

## What this repo currently mirrors

- `backend/searcher-rs|sim-ctl|relays-client|recon` — Rust implementations in the
  spirit of `ARBITRAGEX-CONTABO-BACKEND`.
- `backend/selector-api`, `backend/api-server` — TypeScript services, pragmatic addition
  (control-plane) where hot-path latency is not critical.
- `edge/worker/` + `edge/dev-local/` — Cloudflare Worker (canonical) + dev-only Express shim.
- `frontend/` — Next.js 14 App Router (different stack from `show-my-github-gems`
  specifically to avoid importing its mock legacy).
- `automation/scripts/` — follows ARBITRAGEXPLUS2025 patterns (bash + checklists).
- `database/migrations/` — versioned SQL; no ORM lock-in.
