# OMEGA Opportunity Grid — Feature Module

Alternate visual surface for `/opportunities`, presented as a **card grid**
instead of the dense table. Mounted at `/opportunities-grid`.

Both surfaces read from the **same Omni-Store**; the grid is purely additive.

---

## Attribution

The **operational anatomy** of this module is inspired by the public terminal
of **[DeFiBot.trade](https://defi-bot.trade/)**, specifically:

- Opportunity card layout (token + pair + spread badge + BUY/SELL DEX route +
  investment / net profit / gas trinity + gated CTA).
- Quick-preset selector (Conservative / Balanced / Aggressive) with
  spread bands.
- Terminal stats strip (4 cards: P&L, Win Rate, Volume, Best Trade).
- AI-agent status banner (simulator / executor / notifier / breaker).

**What we intentionally did NOT copy:**

- No colours, cyan accents, wallets, addresses, opcodes or code paths from
  DeFiBot were replicated. The TradePro OKLCH electric-royal-blue theme is
  preserved end-to-end.
- No fonts, icons, images or brand marks from DeFiBot are used.
- The DeFiBot backend, protocol, and any of its execution logic are **not**
  referenced or reimplemented. Only the visual/UX anatomy is adapted.

---

## Files

| File | Purpose |
|------|---------|
| `OpportunityCard.tsx`  | Single card — token + pair + spread + route + PnL + gated CTA |
| `OpportunityGrid.tsx`  | Responsive 1→5 column grid + honest empty state |
| `TerminalStatsBar.tsx` | 4-card stats strip; renders `—` when telemetry is null |
| `PresetSelector.tsx`   | Conservative / Balanced / Aggressive cards |
| `AiAgentBanner.tsx`    | Declares which agents are configured (fail-honest) |
| `__tests__/OpportunityCard.test.tsx` | Vitest static-markup tests (no jsdom) |

Page entry: `app/opportunities-grid/page.tsx` (server) →
`app/opportunities-grid/OpportunitiesGridClient.tsx` (client integrator).

---

## OMEGA_BENCHMARK_30_E2E mapping

This module lifts the following benchmark features from **pending** to
**wired-in-UI** (execution-side wiring tracked separately):

| Feature ID | Description                              | Where it surfaces          |
|------------|------------------------------------------|----------------------------|
| **F-51**   | Strategy Router                          | `strategy_kind` pill on every `OpportunityCard` (dex_arb / triangular / backrun / liquidation / flashloan_arb) |
| **F-54**   | Operational Presets                      | `PresetSelector` with three descriptors (`spread_min_pct` / `spread_max_pct` bands from XLSX) |
| **F-71**   | Public verifiable stats                  | `TerminalStatsBar` — currently `null` on purpose; will hydrate once `/api/stats/public` is reconciliation-backed |

Features **partially exposed** (visible in UI, awaiting backend wire):

- F-31 Executor telemetry → `AiAgentBanner.agent_executor`
- F-32 Simulator telemetry → `AiAgentBanner.agent_simulator`
- F-33 Notifier telemetry → `AiAgentBanner.agent_notifier`
- F-40 Circuit-breaker telemetry → `AiAgentBanner.agent_breaker`

---

## Doctrine markers (must remain green)

### R1 — Mounted Snapshot
- No `Date.now()`, `Math.random()` or `window` reads inside render.
- Server passes `initialSnapshot`; client subscribes to the Omni-Store only
  *after* mount via `useOmniOpportunities`.

### R8 — Fail-Honest
- **CTA gate:** `OpportunityCard` disables the primary action unless
  `status ∈ {validated, simulated, scored}`. `detected` never executes.
- **Stats:** `TerminalStatsBar` renders `—` (not `$0.00`) when the metric
  is `null`, with a caption explaining that gates are closed until the
  first reconciled execution.
- **Filter:** `PresetSelector`'s spread band filter keeps cards with
  **unknown ROI visible** rather than hiding them behind a false "no match"
  state.
- **Agent flags:** `AiAgentBanner` reads only declared
  `NEXT_PUBLIC_AGENT_*` env flags — we never infer agent state from proxy
  signals.

---

## Local run

```bash
cd frontend
pnpm dev -p 5173
# open http://localhost:5173/opportunities-grid
```

To toggle agent banner flags in dev:
```bash
NEXT_PUBLIC_AGENT_SIMULATOR=true \
NEXT_PUBLIC_AGENT_EXECUTOR=false \
NEXT_PUBLIC_AGENT_NOTIFIER=true \
NEXT_PUBLIC_AGENT_BREAKER=true \
pnpm dev -p 5173
```

Tests:
```bash
pnpm vitest run features/opportunities-grid
```

---

## Safety / Ghost-Protocol compliance

- No wallets, private keys, mnemonics, RPC endpoints or transaction
  payloads live in this module.
- No third-party addresses or contract bytecode was copied from DeFiBot
  or any other source.
- All external data flows through the existing `/api/opportunities/live`
  edge endpoint — no direct RPC/mempool taps introduced here.
