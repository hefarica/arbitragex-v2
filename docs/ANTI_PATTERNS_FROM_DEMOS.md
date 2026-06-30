# Anti-Patterns From "DeFi Bot" Demos — What We Rejected and Why

> Institutional memory. Source: 2026-06-29 analysis of the YouTube "DeFi/MEV bot"
> genre (channel @sacarrec, video *"Crypto Arbitrage Bot | DeFi Trading Bot
> Tutorial"*). We harvested the genre's **UX/observability shell** and rejected
> its **strategies and numbers**. This file exists so a future contributor (or a
> future session) does not re-introduce a rejected pattern thinking it's new.

## TL;DR
ArbitrageX v2 is already technically ahead of these demos in ~13/16 dimensions
(real mempool scanner, REVM simulation, 3-relay submission, multichain, 17-gate
pre-execute checklist, 8 Grafana dashboards, paper-mode-by-default). The only
transferable value was **front-end presentation**, re-implemented natively behind
our gates. Everything below is **out of scope by doctrine**, not by oversight.

## Rejected outright (forbidden)

| Pattern in the demo | Why rejected | Gate |
|---|---|---|
| One-click **Execute Sandwich** | Buy-front + sell-back around a user swap is predatory. | `arbx-mev-ethics-gate` → PROHIBIDO |
| One-click **Execute Front-Run** ("insert tx ahead") | Frontrunning a specific user's pending tx for worse fill. | `arbx-mev-ethics-gate` → PROHIBIDO |
| **Back-run-against-user** framed as profit | Only allowed when residual arb after the user already settled; the demo's framing extracts from the originator. | `arbx-mev-ethics-gate` |
| One-click "Execute" with **no simulation** | Any execute path must re-run REVM sim and honor paper-mode. | `arbx-simulation-mandatory`, `arbx-pre-execute-checklist` |
| Same-token "arbitrage" (USDC/USDC, USDT/USDT) | Meaningless; a real monitor never emits it. Fabrication tell. | RULE 00 Zero-Mocks |
| Guaranteed/"crazy profits", ROI to the cent, estimate==actual | Promissory + fabricated (pre-trade estimate equalled post-trade actual; block heights didn't match the selected chain). | Fail-honest (R8), no-guaranteed-returns |

## Deferred (premature on a paper-mode, mainnet-refused system)

Operator decision 2026-06-29: **defer both**, revisit only after mainnet-readiness
certification.

- **On-chain Auto-Compounder contract** — auto-reinvesting funds is an auto-loop on
  an irreversible on-chain action ("jamás auto-recovery"), a new fund-custody
  contract requiring all 7 risk caps + kill-switch + on-chain pause guardian, on a
  system where mainnet is physically refused. Compatible alternative if ever
  wanted: a **reversible off-chain paper-accounting** toggle (re-stakes realized
  *paper* PnL into the next *paper* position, zero custody).
  Gates: `arbx-risk-limits-enforcement`, `arbx-mev-ethics-gate`, global §4.
- **Referral contract** — a fund/payment contract with incentive economics on an
  unlaunched MEV bot carries securities/legal exposure; "non-MLM" does not make a
  premature fund contract safe. Compatible far-future alternative: off-chain
  attribution only, no on-chain payment, no fee.

## Reframed (kept the intent, removed the hook)

- **"Real-time profit calculator"** → **Historical Performance Explorer**
  (backward-only). Shows realized *paper*-PnL distribution (median, p25/p75, max
  drawdown, worst day) + estimate-vs-actual delta. **No forward earnings number.**
  A forward calculator is the genre's #1 conversion hook and would imply the
  promise we forbid.
- **"Time-to-first-trade < 5 min"** growth metric → **time-to-first-*paper*-trade**.
  We do not optimize speed-to-capital on a mainnet-refused system.

## Harvested (doctrine-compatible, in the adaptation sprint)
Operator presets (as the 7 risk caps), onboarding wizard (paper), mempool
telemetry v2, latency-budget tracking + Grafana panel, sim↔paper-exec parity,
Kelly-bounded sizing, live paper-trade execution stepper, rich paper-trade
receipt, confidence % on cards.

## The one rule that catches all of the above
If a feature shows or implies an **earnings outcome**, requires **custody/auto-loop
of funds**, or orders/extracts around a **specific user's pending tx** — stop. It
is forbidden or premature here, regardless of how polished the demo made it look.
