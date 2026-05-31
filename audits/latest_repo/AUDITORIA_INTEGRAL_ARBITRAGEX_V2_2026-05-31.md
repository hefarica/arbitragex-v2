# Auditoría integral del repositorio ArbitrageX v2

**Repositorio auditado:** `https://github.com/hefarica/arbitragex-v2.git`**Rama:** `main`**HEAD auditado:** `4aa5490bcb98e88cf7b8e98b6d483d77e8c7c66a`**Merge visible:** `Merge pull request #115 from hefarica/feature/topology-vault-rpc-mux` — `feat: Omni-Store, Topology Vault, Security Hardening`**Fecha de auditoría:** 2026-05-31**Modo de auditoría:** diagnóstico pasivo, sin modificación del código fuente.

> **Conclusión ejecutiva.** El repositorio más reciente ya incorporó avances reales de Fase 8: **Omni-Store con Zustand**, **Topology Vault backend con validación/redacción**, endurecimiento de sesión admin mediante **cookie httpOnly**, cliente API fail-honest con Zod, y build frontend funcional. Sin embargo, todavía no está listo para live trading: existe segmentación de stores, deuda de migración en registry/wallet/markets/engines, dependencias de toolchain que bloquean comandos `pnpm` normales, y validación Rust no pudo ejecutarse en este sandbox por ausencia de `cargo`. El estado correcto hoy es **paper-only / NO-GO operacional**, con frontend buildable y pruebas frontend verdes.

## 1. Línea base auditada

La auditoría se ejecutó sobre una copia fresca en `/home/ubuntu/arbitragex-v2-latest-audit`. El repositorio contiene una base monorepo amplia: frontend Next.js, backend Node/Rust, contratos Solidity, migraciones SQL, Docker/Compose, documentación doctrinal y artefactos OMEGA.

| Dimensión | Estado observado | Evidencia |
| --- | --- | --- |
| Markdown/docs | 2.970 archivos `.md` | `audits/latest_repo/01_arch_inventory.txt` |
| Rust | 214 archivos `.rs` | `audits/latest_repo/01_arch_inventory.txt` |
| TypeScript | 213 archivos `.ts` y 181 `.tsx` | `audits/latest_repo/01_arch_inventory.txt` |
| SQL | 78 archivos `.sql` | `audits/latest_repo/01_arch_inventory.txt` |
| Solidity | 46 archivos `.sol` | `audits/latest_repo/01_arch_inventory.txt` |
| Docker/Compose | Dockerfiles y compose para frontend, api-server, searcher, recon, relays, sim-ctl, token-enricher, edge | `audits/latest_repo/01_arch_inventory.txt` |
| Migraciones DB | 001–074 visibles, incluyendo topology/readiness/credentials/omni registries | `audits/latest_repo/01_arch_inventory.txt` |

## 2. Validaciones ejecutadas

El frontend **sí compila** por binarios locales y el typecheck pasa. La ruta estándar con `pnpm run ...` queda afectada por scripts de build ignorados por pnpm y por scripts raíz inexistentes bajo los nombres simples `lint`, `typecheck`, `build`.

| Validación | Resultado | Lectura técnica |
| --- | --- | --- |
| `pnpm install --frozen-lockfile` en raíz | **PASS** | Lockfile raíz utilizable; pnpm advierte que `workspaces` en `package.json` no sustituye `pnpm-workspace.yaml`. |
| `pnpm run lint` raíz | **FAIL toolchain/script** | No existe script `lint`; sugiere `lint:all`. |
| `pnpm run typecheck` raíz | **FAIL toolchain/script** | No existe script `typecheck`; sugiere `typecheck:all`. |
| `pnpm run build` raíz | **FAIL toolchain/script** | No existe script `build`; sugiere `build:all`. |
| Frontend vía pnpm | **FAIL pnpm approvals** | `ERR_PNPM_IGNORED_BUILDS` por `esbuild`, `sharp`, `unrs-resolver`; requiere política explícita de `pnpm approve-builds`. |
| `frontend/.bin/tsc --noEmit` | **PASS** | TypeScript estricto válido. |
| `frontend/.bin/next build` | **PASS** | Next.js 16.2.6 compila, genera 22 páginas estáticas y rutas dinámicas. |
| `frontend/.bin/eslint` | **FAIL por warnings** | 0 errores, 3 warnings; política `--max-warnings=0` vuelve rojo el resultado. |
| `frontend/.bin/vitest` | **PASS** | 27 archivos de prueba, 223 tests verdes. |
| `cargo fmt/check` | **NO EJECUTADO** | `cargo: command not found` en sandbox; requiere toolchain Rust instalado. |

## 3. Arquitectura end-to-end observada

El repo se organiza como una plataforma multi-servicio. El frontend Next.js consume un edge/API con contratos tipados; el backend mezcla api-server Node/TypeScript, servicios Rust, simulación, recon, relays, searcher y componentes compartidos. La base de datos PostgreSQL está muy evolucionada, con migraciones dedicadas a oportunidades, ejecuciones, scoring, risk events, audit logs, chains runtime, runtime ACK, operator sovereignty, topology y strategy catalogs.

| Capa | Componentes principales | Estado de madurez |
| --- | --- | --- |
| Frontend | Next.js App Router, rutas admin, readiness, topology, credentials, opportunities, risk, recon, strategies, wallets | **Buildable**, pruebas verdes, nueva adopción Zustand parcial. |
| Edge/API client | `frontend/lib/api-client.ts`, schemas Zod, cookies admin, retries GET, timeouts | **Maduro**, fail-honest y tipado. |
| Admin/session | `frontend/lib/admin-token.ts`, edge session, cookie httpOnly, TTL companion | **Endurecido**, ya no expone token real a JS. |
| Backend API | `backend/api-server`, readiness, topology vault, credentials, risk circuit breakers | **Operacionalmente avanzado**, con blockers explícitos. |
| Rust backend | `searcher-rs`, `shared-rs`, `recon`, `sim-ctl`, `simulator-v2`, `relays-client` | **No validado en sandbox** por falta de Cargo; requiere CI/Rust local. |
| Contratos | Foundry + contratos Solidity | **Inventariado**, no ejecutado en esta auditoría por alcance/toolchain. |
| Infra | Dockerfiles, compose prod/dev/staging, Prometheus/Grafana en despliegue previo | **Presente**, requiere validación reproducible con compose. |

## 4. Hallazgos críticos y de alta prioridad

### H1 — Hay avance real de Omni-SSOT, pero no existe un único store universal

El último repo ya no está en el estado anterior sin Zustand. Ahora existen al menos dos stores relevantes. El primero es `frontend/lib/store/omni-store.ts`, documentado explícitamente como **OMEGA OMNI-STORE — Single Source of Truth**, usando Zustand con slices de registry, opportunities y wallet. El segundo es `frontend/store/useSystemStore.ts`, también descrito como **Single Source of Truth for the Readiness Pipeline**, persistido en `localStorage` para topology, credentials, markets y engines.

| Store | Dominio declarado | Persistencia | Estado real |
| --- | --- | --- | --- |
| `useOmniStore` | Registry, Opportunities, Wallet | No persistente | Parcialmente adoptado; opportunities está conectado, registry aún tiene TODO placeholder, wallet no muestra adopción amplia. |
| `useSystemStore` | Readiness pipeline: topology, credentials, markets, engines | `localStorage` bajo `arbx-system-store` | Control-plane real; separa Step 1–4 y deriva credenciales requeridas por chain. |

**Riesgo:** la arquitectura ya tiene SSOT por dominio, pero el término Omni-SSOT puede inducir a asumir una fuente única global. En la práctica hay **SSOT segmentado**: live opportunities en Omni-Store y readiness pipeline en SystemStore. Esto no es necesariamente incorrecto, pero debe documentarse como contrato oficial para evitar stores paralelos accidentales.

**Evidencia:** `frontend/lib/store/omni-store.ts` define Zustand y slices en líneas 3–17 y crea `useOmniStore` en líneas 124–215. `frontend/store/useSystemStore.ts` define el flujo Step 1–4 en líneas 3–14, estado de topology/credentials/markets/engines en líneas 44–73 y persistencia en líneas 125–203.

### H2 — `omni-store.ts` contiene una inconsistencia tipada de estado WebSocket

`WsStatus` se define como `"CONNECTING" | "LIVE" | "STALE" | "POLLING"`, pero el estado inicial usa `"DISCONNECTED"`. TypeScript aun así pasó, lo cual sugiere que esta rama puede no estar siendo estrictamente verificada en esa sección por inferencia/middleware o que la tipificación quedó relajada en el wrapper.

**Riesgo:** estados UI divergentes entre socket lifecycle y store. Se recomienda normalizar `WsStatus` agregando `"DISCONNECTED"` oficialmente o cambiando el inicial a `"STALE"`/`"CONNECTING"` según doctrina.

### H3 — Registry slice del Omni-Store aún no es fuente real de datos

`fetchRegistry( )` todavía contiene `TODO: Replace with actual API call` y marca `registryStatus: "ready"` sin cargar chains/dexes/pools. Esto contradice parcialmente el objetivo de SSOT canónico para registries.

| Dominio | Estado actual | Acción requerida |
| --- | --- | --- |
| Chains | Existe en SystemStore/topology y APIs/hook `useChains`; no consolidado en Omni registry | Mover snapshot validado a registry slice o declarar SystemStore como fuente de topology. |
| DEXes | UI/API separadas, `DexRegistryClient` mantiene estado local | Integrar `dexes` map desde API canonical. |
| Pools | `PoolsTab` todavía tiene fetch/estado local | Integrar `pools` map desde API canonical. |

### H4 — `useOmniOpportunities` todavía contiene fallback localhost explícito

El hook usa `process.env.NEXT_PUBLIC_WS_URL ?? "http://localhost:3000"`. Aunque `next.config.js` y `api-client.ts` ya bloquean localhost en producción para endpoints públicos, este fallback local debe alinearse con `getWsBaseUrl( )` para evitar una regresión en despliegues no cubiertos por la validación.

**Riesgo:** mismatch frontend/WS en producción si el entorno no define `NEXT_PUBLIC_WS_URL` y la ruta no es interceptada por validación.**Acción:** reemplazar fallback directo por `getWsBaseUrl()` o por error fail-honest en producción.

### H5 — Toolchain raíz no está estandarizada para validación CI/local

Los comandos esperados por operadores (`pnpm run lint`, `pnpm run typecheck`, `pnpm run build`) fallan en raíz porque los scripts reales parecen estar bajo nombres `*:all`. Además, pnpm bloquea frontend con `ERR_PNPM_IGNORED_BUILDS` hasta aprobar builds de dependencias nativas.

**Riesgo:** operadores pueden interpretar fallas de toolchain como fallas de código o, peor, saltarse validación.**Acción:** documentar una matriz oficial de comandos y versionar la política de `pnpm approve-builds`/`.pnpm-approvals.json` para CI reproducible.

## 5. Seguridad, secretos y Fail-Honest

La postura de seguridad mejoró de forma sustantiva en los últimos cambios. El frontend ya no conserva el token admin real en almacenamiento JS; el contrato usa sesión httpOnly y una cookie compañera solo para TTL. El cliente API usa `credentials: "include"`, timeouts, retries solo para GET y Zod para rechazar drift de contrato.

| Control | Estado | Evidencia |
| --- | --- | --- |
| Admin token no legible por JS | **Implementado** | `admin-token.ts` líneas 3–20, 34–45, 48–65. |
| Cookie httpOnly + SameSite Strict | **Implementado por contrato frontend/edge** | `admin-token.ts` líneas 8–12. |
| Cliente API no fabrica datos | **Implementado** | `api-client.ts` líneas 4–13 y 109–208. |
| Producción no acepta localhost API | **Implementado** | `next.config.js` líneas 9–16; `api-client.ts` líneas 34–41. |
| CSP dinámico sin localhost hardcoded | **Implementado** | `next.config.js` líneas 24–45 y 55–73. |
| Readiness no expone secretos crudos | **Implementado en blockers/env probes** | Evidencias en `06_rpc_readiness_credentials_focus.txt`. |

> **Juicio Fail-Honest:** el repo se está moviendo correctamente hacia “fallar visible” en lugar de simular readiness. La existencia de blockers para `RPC_HTTP_1`, `EXECUTOR_1`, paper shadow, fork validation y GO/NO-GO formal indica que el sistema no debería promoverse a live sin prerequisitos explícitos.

## 6. Frontend: performance, rutas y UI

El build Next.js directo produjo rutas estáticas y dinámicas sin errores. La app tiene rutas críticas para `/admin/topology`, `/settings/credentials`, `/live-readiness`, `/opportunities`, `/dex-registry`, `/strategies`, `/wallets`, `/risk`, `/recon`, `/operations`, `/worker-health` y módulos OMEGA S5. El lint solo reporta warnings, pero la política los trata como fallo.

| Warning | Archivo | Severidad | Acción recomendada |
| --- | --- | --- | --- |
| Uso de `<img>` en vez de `next/image` | `frontend/components/TokenChip.tsx` | Baja/media por LCP | Migrar a `Image` o justificar excepción. |
| Dependencia faltante en `useEffect` | `frontend/lib/hooks/useContracts.ts` | Media | Memoizar `fetchData` o ajustar dependencia. |
| Dependencia faltante en `useCallback` | `frontend/lib/useWebSocket.ts` | Media | Incluir `protocols` o estabilizarlo con `useMemo`. |

La migración de opportunities al store es positiva: `OpportunitiesClient` usa `useOmniOpportunities` y selectores de `useOmniStore`. Aun así, `DexRegistryClient`, `PoolsTab` y `WalletsClient` mantienen estado local significativo. Esto es aceptable para UI local, pero no para datos canónicos de dominio.

## 7. Mapa SSOT v8 actualizado del último repo

| Dominio | Fuente de verdad actual | Estado | Riesgo |
| --- | --- | --- | --- |
| Topology Vault | Backend `topology-vault` + `useSystemStore.topology` en frontend | **Parcialmente canónico** | Debe garantizarse rehidratación y reconciliación con snapshot server-side. |
| Credentials | Backend credentials API + `useSystemStore.credentials` como estado UI | **Canónico server-side; UI derivada** | No persistir secretos; solo status, como está. |
| Opportunities | `useOmniStore.opportunities` alimentado por `useOmniOpportunities` | **Canónico frontend parcial** | Fallback WS localhost y polling con clear/repopulate. |
| Registry chains/dexes/pools | APIs/hooks/componentes dispersos; registry slice placeholder | **No consolidado** | Principal deuda Omni-SSOT. |
| Wallets | `useOmniStore.wallet` declarado; `WalletsClient` usa API/local | **No consolidado** | Riesgo de doble verdad UI/store. |
| Markets | `useSystemStore.markets` | **Derivado/local** | Debe ser derivado de gates/backend, no manual. |
| Engines/strategies | `useSystemStore.engines` + pages strategies | **Derivado/local** | Debe vincularse a readiness backend y strategy catalog. |
| API contracts | `frontend/lib/api-client.ts` + schemas Zod | **Fuerte** | Mantener schema-first y no bypass fetch ad hoc. |
| Admin session | Cookie httpOnly gestionada por edge | **Fuerte** | Verificar SameSite/Secure en HTTP vs HTTPS real. |
| Risk/readiness | Backend readiness/extras/circuit breakers | **Fuerte** | Rust/metrics reales deben validarse en CI/VPS. |

## 8. Plan de remediación priorizado

### Bloque A — Validación reproducible de toolchain

Debe corregirse primero porque sin validación confiable no hay release disciplinado. La raíz debe exponer comandos canónicos o documentar los `*:all` como únicos comandos válidos. También debe resolverse `pnpm approve-builds` para `esbuild`, `sharp` y `unrs-resolver` de forma versionada.

| Acción | Comando objetivo |
| --- | --- |
| Validación raíz oficial | `pnpm run lint:all && pnpm run typecheck:all && pnpm run build:all` |
| Frontend directo | `cd frontend && ./node_modules/.bin/tsc --noEmit && ./node_modules/.bin/next build && ./node_modules/.bin/vitest run` |
| Política pnpm | Revisar/versionar `.pnpm-approvals.json` y ejecutar `pnpm approve-builds` bajo control de cambios. |
| Rust | Instalar toolchain y ejecutar `cargo fmt --check && cargo check --workspace && cargo test --workspace`. |

### Bloque B — Formalizar SSOT segmentado

Se recomienda no forzar un único store universal si eso degrada claridad. La arquitectura debe declarar explícitamente dos dominios: **Control-plane SSOT** (`useSystemStore` ) y **Data-plane/UI live SSOT** (`useOmniStore`). Después, mover registry/wallet/pools/dexes a uno de esos dominios sin duplicidad.

| Paso | Resultado esperado |
| --- | --- |
| B1 | ADR “SSOT segmentado: SystemStore vs OmniStore”. |
| B2 | `fetchRegistry()` consume API real y llena chains/dexes/pools. |
| B3 | `DexRegistryClient` y `PoolsTab` leen registry desde store/selectores, no desde fetch/local state canónico. |
| B4 | `WalletsClient` decide: wallet runtime en OmniStore y datos backend en API schema, sin doble verdad. |

### Bloque C — Cerrar warnings de lint

El repo ya está cerca de verde. Corregir los tres warnings permite que `eslint --max-warnings=0` vuelva a ser una señal fuerte.

### Bloque D — Endurecer WS y endpoint resolution

Eliminar el fallback local en `useOmniOpportunities` y reutilizar `getWsBaseUrl()` o una validación fail-fast equivalente. Esto alinea el hook con `next.config.js` y `api-client.ts`.

### Bloque E — Validación backend/Rust/contratos en entorno correcto

Esta auditoría no pudo compilar Rust por ausencia de Cargo. Antes de cualquier live-readiness real, debe correr la suite completa en un entorno con Rust, Foundry y servicios Docker.

| Área | Validación mínima |
| --- | --- |
| Rust | `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`. |
| Fork validation | Test ignorado multistep/fork con `RPC_HTTP_1 + EXECUTOR_1`, sin mocks. |
| Contracts | `forge test` y gas/safety reports. |
| Docker | `docker compose -f docker/compose.prod.yml config` y smoke test de servicios. |
| Readiness | 16 gates, circuit breakers, paper-shadow y GO/NO-GO formal. |

## 9. Estado GO/NO-GO

| Criterio | Estado | Decisión |
| --- | --- | --- |
| Frontend build | Verde por binario local | Apto para staging UI. |
| Frontend tests | Verde, 223/223 | Apto para staging UI. |
| Lint | Amarillo, 3 warnings tratados como fallo | Corregir antes de release formal. |
| Backend/Rust | No validado en sandbox | Bloqueante para live. |
| Toolchain raíz | Amarillo/rojo por scripts/pnpm approvals | Bloqueante para CI limpio. |
| Secrets/admin | Mejorado | Mantener pruebas V-AT-1. |
| RPC/readiness | Fail-honest diseñado | Requiere validación real en VPS/CI. |
| Live trading | **NO-GO** | Mantener paper-only hasta A.4–A.9 completas. |

## 10. Entregables de evidencia

Los artefactos completos quedaron bajo:

`/home/ubuntu/arbitragex-v2-latest-audit/audits/latest_repo/`

| Archivo | Contenido |
| --- | --- |
| `00_repo_metadata.txt` | Remote, rama, HEAD, commit y estructura top-level. |
| `01_arch_inventory.txt` | Inventario por extensión, manifiestos, Docker, migraciones y frontend. |
| `03_architecture_surface.txt` | Superficies de arquitectura y endpoints. |
| `04_validation_summary.txt` | Validaciones pnpm/root/frontend iniciales. |
| `04b_direct_validation_summary.txt` | Validaciones directas con binarios locales. |
| `04c_test_summary.txt` | Resultado Vitest: 27 archivos, 223 tests. |
| `05_security_rpc_docker_ci.txt` | Auditoría amplia de seguridad/RPC/Docker/CI. |
| `06_rpc_readiness_credentials_focus.txt` | Evidencia focal de RPC, readiness y credentials. |
| `07_frontend_omni_ssot_performance_ui.txt` | Auditoría frontend Omni-SSOT/performance/UI. |
| `08_frontend_store_focus.txt` | Evidencia de stores, hotspots y adopción real. |

## 11. Recomendación final

El último merge representa una mejora real y sustancial. La prioridad ya no es “crear” la arquitectura, sino **cerrar la brecha entre intención doctrinal y validación reproducible**. El siguiente sprint debe enfocarse en: comandos CI deterministas, cierre de warnings, consolidación de registry/pools/dexes en SSOT, eliminación de fallback localhost en WS, y ejecución de la suite Rust/Foundry/Docker en un entorno con toolchain completa. Hasta completar esos puntos, el sistema debe permanecer en **paper-only / NO-GO live**.

