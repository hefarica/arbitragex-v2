# WO-04 — CROSS-EXAMINATION (par adversarial del entregable TS)

> Cross-examiner par de WO-04-TS (Gang Omniscience). Fecha: 2026-09-07.
> Objeto: refutar `WO-04-APPLY-TS.md` + los 5 archivos tocados (commit `97742279`).
> Método: TODA la evidencia ниже es propia — re-lectura, re-ejecución de gates,
> git forense, y VPS por ssh `arbx` ESTRICTAMENTE read-only (docker ps/logs/inspect,
> redis-cli GET, psql SELECT, curl localhost). Presupuesto dominio público: 0/5.
>
> Lexico OMEGA: LP fee = fricción de Variedad de Liquidez · gas = fricción
> termodinámica.

## VEREDICTO: **GAPS** — código local honesto y verificado, PERO la cadena de
## release rompió el plano trading-config de PRODUCCIÓN y sigue roto (8+ h).

## 1. Lo que CONFIRMÉ a favor del entregable (refutación fallida, evidencia propia)

1. **Gates re-ejecutados por mí**: `npx vitest run src/simulation/computeSimulatedNet.test.ts
   src/routes/trading-config.test.ts` → **2 files / 30 passed / EXIT 0** (idéntico al claim §3).
2. **Alineación INSERT recontada a mano** (trading-config.ts:577-596 vs 646-681):
   33 columnas ↔ 33 placeholders ↔ 33 params; `lp_fee_default_pct` = columna 23 ↔
   `$23` (L594) ↔ `body.lp_fee_default_pct` param 23 (L669). EXACTA.
3. **RULE 00 / R8 limpios**: el Caso 1 (test L168-200) usa un doble de CLIENTE
   Redis en la frontera con un blob fixture que atraviesa el `parseSnapshot` REAL;
   aserciones exactas (0.003, 7.05, string `"lp-fee=30bps-proxy"`). No hay mock
   de resultados. Nota dinámica `Math.round(0.003*10_000)===30` byte-idéntica.
4. **Espejo zod** (shared-ts/src/config/index.ts:104-109) presente con `.default(2.0)`;
   grep propio: `priority_fee_gwei` tiene CERO consumidores TS — espejo inerte (ver §3.5).
5. **Claim frontend re-verificada**: `frontend/lib/schemas.ts:504-507`
   `TradingConfigConfiguredSchema = z.object({...})` sin `.strict()` → la clave
   nueva se descarta, el parse frontend sigue verde.
6. Cero `.rs` tocados por la mitad TS (diff de `97742279` = exactamente los 5
   archivos claimados, 123+/8−). §34.3 intacto. La suite completa 729/729 del
   verifier es plausible (no la re-corrí completa; los 30 dirigidos sí).

**Conclusión parcial**: el reporte NO es de humo en lo local. Cada gate citado
es real y lo reproduje. El fallo está en lo que el reporte NO podía garantizar
— y en que su advertencia §4.2 fue ignorada por la cadena de empaquetado.

## 2. REFUTACIÓN MATERIALIZADA — producción rota por la dependencia declarada y omitida

Cadena forense (evidencia propia):

| Paso | Hecho | Evidencia |
|---|---|---|
| 1 | TS committed | `97742279` (22:27 -0500) — 5 archivos, exactamente el diff §6 del reporte |
| 2 | Verifier aprobó con hazard F3 documentado | WO-04-VERIFY.md §3 F3: "migración 119 untracked vs TS committed… si el operador deploya sin la 119, los routes rompen (column does not exist)" |
| 3 | PR #547 mergeó SOLO la mitad TS | `a0bcf29d` (23:14 -0500), merge de `feat/gang-omniscience-2026-09-06` — SIN `database/migrations/119_*` (la migración estaba untracked, territorio de la mitad Rust) |
| 4 | Deploy a producción | VPS `/opt/arbitragex-v2` HEAD = `a60de001` (post-#548, sobre #547) |
| 5 | **Columna AUSENTE en PG prod** | `information_schema.columns` → 0 filas para `trading_config.lp_fee_default_pct` |
| 6 | **GET roto EN VIVO** | `curl 127.0.0.1:8080/api/v1/trading-config?chain_id=1` → **HTTP 503** `{"error":"query_failed","detail":"column \"lp_fee_default_pct\" does not exist"}` |
| 7 | **Path del frontend roto EN VIVO** | `curl 127.0.0.1:8787/api/trading-config?chain_id=1` → **HTTP 503** (ruta que consumen `app/strategies/tabs/DexesTab.tsx:47`, `CapitalRiskTab.tsx`, `features/cartridge/CartridgeFilterPanel.tsx:62`) |
| 8 | **Boot rehydrate muerto** | log api-server 2026-09-07T05:14:16.884Z: `trading_config.rehydrate_failed — column "lp_fee_default_pct" does not exist` (a cada arranque) |
| 9 | PUT admin roto (estático) | INSERT (trading-config.ts:587) lista la columna inexistente → 42703 → el operador NO puede modificar config en absoluto |
| 10 | Fix existe PERO varado | hotfix `f7ed4cdb` (23:37 -0500) añade migración 119, pero SOLO vive en branch local `fix/wo15-xinfo-shape`; `origin/main` (a60de001) NO lo contiene; `run_migrations.sh` jamás corrió en VPS |

Impacto vivo matizado (honesto): el hot path de simulación sigue OK — el blob
Redis espejo es PRE-WO-04 (grep: sin `lp_fee_default_pct`) y el fallback
`num(...,0.003)` (test-pinneado) preserva el comportamiento. PERO:
- GET/PUT trading-config = 503/500 usuario-visible (pestañas de /strategies).
- Redis flush/restart = espejo irrecuperable (rehydrate es el camino roto) →
  searcher `has_config=false` → feed 0 oportunidades — el modo-incidente
  documentado en el propio trading-config.ts:398-405 (root-cause 2026-06-04).

Culpa comparada: el apply report DECLARÓ la dependencia (§4.2: "migración 119
ANTES del deploy del api-server nuevo… el zod default protege el PUT, no el
SELECT") y el verifier la escaló (F3). El empaquetado de #547 la omitió y el
despliegue (#547 y #548) procedió dos veces sin ella. El entregable es honesto;
el proceso de release incumplió SU propia advertencia y 8+ h después nadie ha
remediado teniendo el fix committed localmente.

## 3. Gaps adicionales

1. **[operator-gated] Remediar producción**: merge/push `f7ed4cdb` (o PR de
   migración 119) → deploy → `run_migrations.sh` en VPS. Agentes NO podemos
   (NO-GIT + VPS intocable §32/§33).
2. **[agent-fixable] Board/reporte stale**: fila WO-04 de GOAL-WORKORDERS.md
   sigue "½ TS APPLIED_VERIFIED ✅" sin mencionar la rotura en producción; el
   headline del apply ("invariante de primer deploy — sin cambio de
   comportamiento") es solo verdad para el path de lectura con blob viejo.
   Escribir el post-mortem en esta carpeta y corregir la fila.
3. **[agent-fixable] Hueco de cobertura route-level**: los "4 route tests"
   (trading-config.test.ts) son tests EMIT-04 de `universeFingerprint` — cero
   cobertura de D12 (zod/DbRow/SQL/INSERT); el grep propio no halla
   `lp_fee_default_pct` ni DB en ese archivo. La alineación 33↔33 se verificó
   solo por lectura — y por ese hueco se coló la rotura. Agregar test de
   integración de ruta (pool stub con esquema) o probe de columna al boot.
4. **[agent-fixable] F1 sin corregir**: trading-config.ts:672 comentario stale
   "$25::jsonb" (tras el renumerado es `$26::jsonb`; `$25` es `uuid[]`). Presente
   en el código committed.
5. **[operator-gated/coordination] Espejo antes que la cosa espejada**:
   `priority_fee_gwei` zod default 2.0 YA está en producción (inerte — 0
   consumidores TS verificado) mientras su contraparte Rust (D1/D2/D4/D5) está
   SIN commitear en la working tree. El app.schema.json deployeado
   (`additionalProperties:false`, sin la clave) haría fail-fast el boot si el
   operador pone la clave en app.toml HOY. El knob no es operable hasta que
   aterrice la mitad Rust.

## 4. Restricciones del cross-examiner cumplidas

- CERO git write, CERO VPS mutation (solo docker ps/logs/inspect, redis-cli GET,
  psql SELECT, curl localhost VPS-internal), CERO requests a dominio público (0/5).
- §32/§33/§34.3 intactos por mi parte. Nada de executor/wallets/capital/firma.

## Estado

**GAPS** — entregable local: honesto, verificado y reproducido por el par.
Entregable EN CAMPO: su dependencia dura de deploy fue incumplida por la cadena
#547/#548 y producción sigue con el plano trading-config en 503/500 y el
rehydrate muerto. Se requiere decisión del operador (merge del hotfix +
migración) y 3 remiendos agent-fixable (board/post-mortem, test de ruta, F1).
