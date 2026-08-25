# AUDIT — Scope de controles: VIEW_ONLY / LOCAL_PREFS / RUNTIME_MUTATION (2026-08-24)

**FE-0041** (FE-MASTER P10-DRIFT-HOME §52 §53 §54 §63). Modo: §32 audit +
etiquetado. Todo lo abajo es OBSERVADO con file:line (write-path grep:
`putTradingConfig | method: POST|PUT|DELETE|PATCH`). Cero mocks, cero
clasificaciones inferidas sin evidencia.

## Contrato (§52-§54)

- **§53** `/settings` = SOLO presentación: sus controles escriben exclusivamente
  localStorage del navegador; JAMÁS runtime ni config.
- **§54** `/config/trading` = SSOT de knobs de trading: toda mutación de knobs
  pasa por `putTradingConfig` (Redis → searcher-rs hot-reload ≤1s).
- **§63** todo control se etiqueta con su scope visible
  (`<ControlScopeBadge/>`), y cada etiqueta se explica vía `title`.

## Tres scopes (el tercero es honestidad, no invención)

| Scope | Escribe | Tono |
|---|---|---|
| `VIEW_ONLY` | nada — renderiza el wire | outline |
| `LOCAL_PREFS` | SOLO localStorage (presentación) | info |
| `RUNTIME_MUTATION` | plano de config/admin → runtime | warning |

`LOCAL_PREFS` existe porque llamar "VIEW_ONLY" a /settings sería falso: sí
escribe (localStorage). El título pide dos etiquetas; la tercera es la
clasificación R8-honesta del mismo eje.

## Inventario OBSERVADO

### /settings — `LOCAL_PREFS` ×3 cards (contrato §53 CUMPLIDO)

| Control | Write path |
|---|---|
| Notifications (threshold) | `useUserPrefs` → localStorage (`SettingsClient.tsx:105` savePrefs) |
| Feed & Polling (interval, chain) | idem |
| Display (theme, density) | idem |

Cero `fetch` de escritura en el archivo. Etiqueta aplicada por card.

### /settings/credentials — `RUNTIME_MUTATION` (excepción §53: subruta admin, no prefs)

| Control | Write path |
|---|---|
| Test credential | POST `/admin/credentials/test` (`CredentialsClient.tsx:411-412`) |
| Save credential | PUT `/admin/credentials` (`:457-458`) |
| Delete credential | DELETE (`:496`) |

Nota: /settings/credentials es superficie ADMIN (MC-CRED), no preferencias;
el contrato §53 aplica al árbol de prefs (`/settings` raíz). Pendiente
operador: etiquetar esa subruta con badge propio (fuera del scope quirúrgico
de FE-0041 para no tocar superficie de d9 — FE-0010 lane).

### /config — 2 mutaciones aisladas + resto view

| Control | Scope | Write path |
|---|---|---|
| PaperModeToggle | RUNTIME_MUTATION | POST `/admin/config/paper-mode` (`paper-mode-toggle.tsx:90-91`) |
| RpcBackendToggle | RUNTIME_MUTATION | PUT `/api/admin/rpc-backend` vía `useRpcBackend.setBackend` (`hooks/useRpcBackend.ts:114-117`) |
| KV System/Execution/Risk/Scoring + tables Chains/Relays/Breakers | VIEW_ONLY | sin writes (SSR `getConfigCurrent`) |
| CanonicalKnobsPanel (d9 FE-0061) | VIEW_ONLY | "READ-ONLY surface… Mutation contracts do not exist by design" (doc del componente) |

### /config/trading — `RUNTIME_MUTATION` (el SSOT §54)

TradingConfigForm → `putTradingConfig` (`trading-config-form.tsx:222`).
Etiqueta a nivel de página: todo control de la página muta runtime.

### /strategies — 11 tabs (etiquetados en el contenedor, un solo archivo)

| Tab | Scope | Evidencia write |
|---|---|---|
| capital-risk | RUNTIME_MUTATION | `CapitalRiskTab.tsx:78` putTradingConfig ×2 |
| catalog | RUNTIME_MUTATION | monta `StrategyCatalogTab.tsx:132` putTradingConfig ×2 (canon-view + runtime-kinds) |
| runtime | RUNTIME_MUTATION | `RuntimeCartridgesTab.tsx` putTradingConfig round-trip (comment :324) |
| math | VIEW_ONLY | 0 writes |
| dexes | RUNTIME_MUTATION | `DexesTab.tsx:192` ×2 |
| pools | VIEW_ONLY | 0 writes |
| relays | VIEW_ONLY | 0 writes |
| detectors | VIEW_ONLY | 0 writes (DetectorPolicyPanel, EMIT-08 feeds) |
| tokens | RUNTIME_MUTATION | subvista TokenAllowlistTab → `TokenAllowlistTab.tsx:158` ×2 (Universe/QuoteBase son view) |
| simulation | RUNTIME_MUTATION | `SimulationTab.tsx:108` ×2 |
| audit | VIEW_ONLY | 0 writes |

### Otras superficies con writes (inventariadas, etiquetado fuera de scope FE-0041)

- `ServiceControlPanel.tsx:63` POST (service control) — ops.
- `RpcSyncPanel.tsx:119,147` POST `/api/admin/rpcs/reload` — ops.
- `SimulateButton.tsx:71` POST on-demand simulation — efímero, no config.
- `GSimSmokeTestCard.tsx:59,88` POST smoke — ops/diagnostics.
- `/admin/chains` CRUD (`api-client.ts:389-483`) — admin registry (POST/PUT/DELETE).
- `/omega-s5/registry/[entity]` — botones Agregar/Editar/Deshabilitar
  pre-existentes NO funcionales (`setSelectedId` voided, `RegistryPageClient.tsx`);
  cuando se wiren, DEBEN nacer etiquetados RUNTIME_MUTATION y pasar por
  `useActionState` (§56, wiring FE-0040 ya presente).

## Hallazgos

1. **El SSOT §54 es real**: TODA mutación de knobs de trading del app tree
   vivo pasa por `putTradingConfig` (7 sitios, un solo client path). Sin
   escrituras paralelas.
2. **§53 cumplido**: /settings (prefs) cero writes de red; etiquetado
   LOCAL_PREFS ×3.
3. **Etiquetado aplicado**: ControlScopeBadge (puro, server+client) en
   /settings (3), /config (2 RM + 1 VO), /config/trading (1 RM), /strategies
   (11 tabs) = 18 etiquetas.
4. `app_backup/` replica surfaces viejas — excluido del inventario vivo (no
   se sirve); limpiarlo es tarea de housekeeping aparte.

— 7b, sesión arbitragex-v2-main-17-7b, cero commits (PROTOCOLO ABSOLUTO).
