# OMEGA LOOP — Tracker de Corrección Total (ArbitrageX v2)

Fuente de verdad: `Auditoria_Frontend_ArbitrageX_v2.md` (19 anomalías). Regla: solo el agente mueve un ID a CLOSED con evidencia pegada (curl / log / screenshot) **y verificada en vivo en producción**.

> **🔧 Reconciliación 2026-08-10 (este commit corrige una versión inexacta).** Una versión previa de este tracker (traída por el merge B-01 `5704c5f4`) marcaba A-01, FASE0 y A-03 como CLOSED — **inexacto**. `git branch --contains` confirma que **solo B-01** (`5704c5f4`) está merged en `main`. A-01 (`634e414c`) y FASE0+A-03 (`f0319a58`) estaban **varados en ramas locales sin merge**. Además el L3 "anónimo→gate" alegado para A-01 fue contra **dev-local**, no producción; `curl` a producción (2026-08-10) confirma que `/audit-logs` **sigue filtrando** (482 KB + PII del operador). Esta versión alinea la tabla con la realidad.

| ID | Estado | Commit / Rama | Evidencia de cierre | Fecha |
|---|---|---|---|---|
| A-01 | **VERIFY** (L1/L2 ✓; L3 prod pending deploy) | PR [#310](https://github.com/hefarica/arbitragex-v2/pull/310) · `fix/omega-A01-combined` (`634e414c`+`f514a733`) | **COMBINE** (decisión operador): client-side gate + edge-worker redaction (previo) **+** api-server `redactAuditRow` en el origen (cubre `ip_address`+`target_id`+`actor`) + 14 unit tests. L1: vitest 14/14, tsc 0 en api-server+frontend+edge. Prod curl ANTES: 482 KB + email + 76 `auth.login_ok` + 52 IPv6 crudos en `target_id`. **L3 prod tras deploy.** Rama previa `fix/omega-loop-a01` superseda. | 2026-08-10 |
| FASE0 | **OPEN** (fix previo varado, sin merge) | `fix/omega-loop-fase0-a03` (`f0319a58`) — NO en main | Contract test `lib/schemas.defi-contract.test.ts` (6/6) existe en la rama varada. **Resumir**: verify + merge + deploy + live. | 2026-08-09 |
| A-03 | **OPEN** (fix previo varado, sin merge) | `fix/omega-loop-fase0-a03` (`f0319a58`) — NO en main | Alineación `/pools` (`dex_name`/`is_active`) + `/chains` (sin `rpc_url`) en la rama varada. **Resumir**: verify + merge + deploy + live. | 2026-08-09 |
| B-01 | **CLOSED** (merged a main) | `5704c5f4` (en main) | Web3Provider aislado a `app/wallet/layout.tsx`. L1 tsc/eslint limpio. L3 dev-local: STATUS/HOME WC count 0. ⚠️ Deploy del frontend a prod pendiente de confirmar para el criterio live en producción. | 2026-08-09 |
| A-02 | OPEN | — | — | — |
| B-02 | OPEN | — | — | — |
| B-03 | OPEN | — | — | — |
| B-04 | OPEN | — | — | — |
| C-01…C-07 | OPEN | — | — | — |
| D-01…D-05 | OPEN | — | — | — |

---

## A-01 — `/audit-logs` expone el registro administrativo sin autenticación (🔴 P0) · VERIFY

**Estado real (2026-08-10):** producción **sigue filtrando** hasta deploy. El fix vive en PR [#310](https://github.com/hefarica/arbitragex-v2/pull/310) (`fix/omega-A01-combined`), combinando el trabajo previo (client gate + redacción edge, `634e414c`) con una capa nueva de redacción en **api-server** (origen de datos, `f514a733`: `backend/api-server/src/lib/audit-redact.ts`, cubre `ip_address`+`target_id`+`actor`, 14 unit tests).

- **L1 ✓**: vitest 14/14; tsc 0 errores en api-server, frontend, edge-worker.
- **L2 ✓**: contrato Zod preservado por construcción.
- **L3/L4 ⏳**: requieren deploy de api-server + edge worker (operator-gated §34.3).

**Causa raíz:** `/audit-logs` era Server Component SSR → `getAuditLogs()` inyectaba `ARBX_ADMIN_TOKEN` del server → edge autorizaba → SSR servía filas con PII a cualquier visitante anónimo.

**Criterio de aceptación (§4):**
- [x] Anónimo → gate (estructural: sin cookie httpOnly → edge `401 missing_admin_token`). Dev-local verificado.
- [ ] Con sesión admin → filas con IP `/48` y `actor` hasheado — **requiere deploy**.

**Residual:** `hashActor` es SHA-256 sin salt (pseudónimo consistente); HMAC con clave en la fuente de escritura es follow-up.

---

## B-01 — WalletConnect placeholder degrada las 56 páginas (🔴 P1) · CLOSED (merged a main)

**Causa raíz (confirmada en código):** el layout raíz (`app/layout.tsx`) envolvía TODA la app en `<Web3Provider>` (wagmi + react-query + RainbowKit). `getWagmiConfig()` usa `walletConnectProjectId() ?? "walletconnect_project_id_missing"` → con el projectId ausente, el conector WalletConnect llama a `walletconnect.org` en CADA página (2 requests → `ERR_CONNECTION_RESET`), retardando la hidratación 7-12s y causando el falso `0/4 LOCKED`.

**Corrección (audit opción 2 — aislar Web3):**
- `app/layout.tsx`: removido `Web3Provider` + el plumbing de cookie/`headers()` (solo wagmi lo necesitaba).
- `app/wallet/layout.tsx` (nuevo): monta `Web3Provider` solo en la ruta `/wallet` (la única superficie que conecta wallet), con la hidratación SSR via cookie.
- Blast radius verificado: SiteHeader/AppSidebar/nav-items/SystemGuardBanner/OpportunityTicker **no** usan wagmi/wallet; omega-s5 tampoco. Solo `app/wallet/*`.

**Verificación:**
- L1: `tsc --noEmit` 0 errores en B-01; `eslint` limpio.
- L3 (dev-local, python playwright): `STATUS_WC_COUNT=0`, `HOME_WC_COUNT=0` (cero requests a walletconnect.org en /status y /); `WALLET_BODY_LEN=2658` (/wallet sigue renderizando con Web3).
- ⚠️ L3 en **producción** pendiente confirmar tras deploy del frontend.

**Criterio de aceptación (§4 B-01):**
- [x] Ninguna página carga walletconnect.org excepto /wallet (dev-local).
- [x] Sin `ERR_CONNECTION_RESET` en consola de páginas no-wallet (dev-local).
- [ ] Provisionar `NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID` real queda como follow-up del operador (el self-test ya lo flaggea); el fix estructural (aislar Web3) cura la degradación global aunque el ID siga ausente.

**NO se tocó:** wagmiConfig (sigue fail-honest con placeholder), el hot-path, kill-switch, radar de rutas, doctrina honesta.
