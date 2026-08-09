# OMEGA LOOP — Tracker de Corrección Total (ArbitrageX v2)

Fuente de verdad: `Auditoria_Frontend_ArbitrageX_v2.md` (19 anomalías). Regla: solo el agente mueve un ID a CLOSED con evidencia.

> Ramas: el loop avanza por fases en ramas separadas que se mergean a `main`. A-01=`fix/omega-loop-a01` (634e414c); FASE0+A-03=`fix/omega-loop-fase0-a03` (f0319a58); B-01=`fix/omega-loop-b01` (este commit). Reconciliar este archivo al mergear.

| ID | Estado | Commit / Rama | Evidencia de cierre | Fecha |
|---|---|---|---|---|
| A-01 | CLOSED | `634e414c` `fix/omega-loop-a01` | SSR token removido; client-side + gate; edge redacción IP/48 + SHA-256 actor. Anónimo → gate, 0 emails/IPs (playwright). Edge worker tsc 0 errores. Deploy worker pendiente para vista con sesión. | 2026-08-09 |
| FASE0 | CLOSED | `f0319a58` `fix/omega-loop-fase0-a03` | Contract test `lib/schemas.defi-contract.test.ts` (6/6): acepta payload real, rechaza drift `dex/active`. Zod endurecido. | 2026-08-09 |
| A-03 | CLOSED | `f0319a58` `fix/omega-loop-fase0-a03` | `/pools` contrato alineado (`dex_name`/`is_active`); en vivo → 61 pools, 61 ACTIVE, 0 DISABLED, DEX real. `/chains` sin `rpc_url` → columna RPC removida. tsc/eslint limpio. | 2026-08-09 |
| B-01 | CLOSED | _(este commit)_ `fix/omega-loop-b01` | Web3Provider removido del layout raíz; montado solo en `app/wallet/layout.tsx`. En vivo: STATUS_WC_COUNT=0, HOME_WC_COUNT=0 (0 calls walletconnect.org en páginas no-wallet); /wallet renderiza. tsc/eslint limpio. | 2026-08-09 |
| A-02 | OPEN | — | — | — |
| B-02 | OPEN | — | — | — |
| B-03 | OPEN | — | — | — |
| B-04 | OPEN | — | — | — |
| C-01…C-07 | OPEN | — | — | — |
| D-01…D-05 | OPEN | — | — | — |

---

## B-01 — WalletConnect placeholder degrada las 56 páginas (🔴 P1)

**Causa raíz (confirmada en código):** el layout raíz (`app/layout.tsx`) envolvía TODA la app en `<Web3Provider>` (wagmi + react-query + RainbowKit). `getWagmiConfig()` usa `walletConnectProjectId() ?? "walletconnect_project_id_missing"` → con el projectId ausente, el conector WalletConnect llama a `walletconnect.org` en CADA página (2 requests → `ERR_CONNECTION_RESET`), retardando la hidratación 7-12s y causando el falso `0/4 LOCKED`.

**Corrección (audit opción 2 — aislar Web3):**
- `app/layout.tsx`: removido `Web3Provider` + el plumbing de cookie/`headers()` (solo wagmi lo necesitaba).
- `app/wallet/layout.tsx` (nuevo): monta `Web3Provider` solo en la ruta `/wallet` (la única superficie que conecta wallet), con la hidratación SSR via cookie.
- Blast radius verificado: SiteHeader/AppSidebar/nav-items/SystemGuardBanner/OpportunityTicker **no** usan wagmi/wallet; omega-s5 tampoco. Solo `app/wallet/*`.

**Verificación:**
- L1: `tsc --noEmit` 0 errores en B-01 (sin errores en layout/wallet/Web3Provider); `eslint` limpio.
- L3 (en vivo, python playwright): `STATUS_WC_COUNT=0`, `HOME_WC_COUNT=0` (cero requests a walletconnect.org en /status y /); `WALLET_BODY_LEN=2658` (/wallet sigue renderizando con Web3).

**Criterio de aceptación (§4 B-01):**
- [x] Ninguna página carga walletconnect.org excepto /wallet (y omega-s5 que conecten wallet — ninguna lo hace hoy).
- [x] Sin `ERR_CONNECTION_RESET` en consola de páginas no-wallet.
- [ ] Provisionar `NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID` real queda como follow-up del operador (el self-test ya lo flaggea); el fix estructural (aislar Web3) cura la degradación global aunque el ID siga ausente.

**NO se tocó:** wagmiConfig (sigue fail-honest con placeholder), el hot-path, kill-switch, radar de rutas, doctrina honesta.
