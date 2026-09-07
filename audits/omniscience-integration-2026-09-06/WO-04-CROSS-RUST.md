# WO-04 — CROSS-EXAM (mitad Rust): intento de refutación del APPLY-RUST + VERIFY

> Cross-examiner par del agente WO-04. Fecha: 2026-09-07 05:35Z. Método:
> NO heredar ningún número de los reportes — diffs re-leídos, gates re-ejecutados
> con exit codes propios (PowerShell `$LASTEXITCODE`; cargo NO está en PATH de
> Git Bash — gotcha ya documentado en WO-04-VERIFY §Meta, reproducido), y estado
> de producción verificado EN VIVO por ssh read-only (4 probes, 0 requests a
> dominio público).

## VEREDICTO: **PASS** (el entregable sobrevive la refutación; 6 residuos, ninguno invalida el claim)

## 1. Lo que INTENTÉ refutar y NO pude (evidencia propia)

1. **Fidelidad D1-D11**: cada hunk re-leído en el working tree coincide con el
   reporte. Muestras verificadas línea a línea:
   `shared-rs/src/config.rs:138-145+165-168` (serde default 2.0),
   `configs/schemas/app.schema.json:51` (`minimum: 0` bajo `execution`
   con `additionalProperties: false` confirmado),
   `bundle_builder.rs:97-100/109/177-178` (8º param + allow con precedente +
   `(priority_fee_gwei * 1e9).round() as u64` — literal 2_000_000_000 eliminado),
   `submit_engine.rs:446` (+1 línea exacta, el resto del archivo es WO-05),
   `liquidation_worker.rs:143-172` (`DEFAULT_GAS_COST_USD` + `resolve_gas_cost_usd`
   con warn R8), `liquidation_engine.rs` (espejo privado ELIMINADO, `self.gas_cost_usd`),
   `scanner.rs:446-453`, `main.rs:1022`, `trading_config.rs:359-370+424-427`,
   migración 119 (committed `f7ed4cdb`): `ADD COLUMN IF NOT EXISTS … NUMERIC(6,4)
   NOT NULL DEFAULT 0.0030 CHECK (>= 0 AND <= 0.5)`. Los 6 fixtures D10-collateral
   y los 3 tests de integración llevan marcador `// WO-04 (2026-09-06)` (grep: 22
   sitios Rust).
2. **Gates REALES, no de humo** — re-ejecutados por mí sobre el árbol compuesto
   (WO-04 + WO-05 + HEAD hotfix WO-15), cwd `backend/`:
   `cargo check --all-targets` ×4 crates **EXIT 0** (4m32s) · `cargo clippy …
   -- -D warnings` **EXIT 0** · `cargo fmt -- --check` **EXIT 0** ·
   searcher-rs lib **1150 passed / 0 failed / 3 ignored** · relays-client
   **77/0/1** · shared-rs **217+1** · prioritization-spine **121** — suma
   **1566/0**, coincide 1:1 con applier y verifier. La parte Rust del verify
   NO es de humo.
3. **§34.3 intacto**: `live_exec_policy.rs` cero diff; default-deny
   (`:32`,`:53`) y `MainnetRefused` (`:37`,`:85`,`:124`,`:141`,`:154`) presentes;
   barrera M1 como primer statement de `build_and_sign` (`:111-113`) sin tocar.
4. **Invariante primer deploy**: `configs/app.toml:18-27` [execution] NO lista
   `priority_fee_gwei` (verificado por lectura) → serde default 2.0 gobierna;
   env `LIQUIDATION_GAS_COST_USD` ausente → 30.0. Censo `ExecutionCfg {`
   re-ejecutado: solo la definición (serde-only, claim CIERTA).
5. **RULE 00 / R8**: cero datos fabricados; defaults declarativos = literales
   históricos (mandatado por charter); kernel fund-path sigue endurecido
   (`estimate_liquidation_profit` → None para gas no-finito/negativo,
   `liquidation_worker.rs:291-293`); boot log imprime el knob real (`:763`).
6. **Cero regresión fuera de claim**: los extras del tree (deletes `executor/*`,
   `resync_nonce`, `#[allow(dead_code)]` removido) son todos WO-05 marcados y
   documentados en WO-05-APPLY/VERIFY; `git diff scanner.rs` = UN solo hunk WO-04.

## 2. Incidente de producción WO-04-cadena — verificado EN VIVO (cronología propia)

- Deploy `a0bcf29d` (PR #547, mitad TS `97742279`) omitió la migración 119
  pese a la dependencia documentada en 3 lugares (verify F3 la PREDIJO).
- 05:31Z: `GET /api/trading-config` (edge interno VPS) = **503**; columna
  `lp_fee_default_pct` ausente en PG prod; logs api-server:
  `trading_config.read_failed: column "lp_fee_default_pct" does not exist`.
- 05:32-05:33Z: migración 119 aplicada en VPS (vi la columna aparecer entre
  probes) + restart api-server: 500 → **05:33:22Z `trading_config.rehydrated
  enabled_rows:1 mirrored:1`** y endpoint sirviendo datos reales de PG.
- Producción SANA al cierre de este cross-exam. La fila del board
  "gap#1 remedio producción ✓" quedó veraz sólo al final de mi ventana.

## 3. Gaps residuales

| # | Gap | Clase |
|---|---|---|
| 1 | §37 "incidente cierra con revert + gate NUEVO": NO existe gate de empaquetado que verifique que código TS dependiente de SQL viaja CON su migración (el 503 fue exactamente esa omisión). Diseño del gate = agent-fixable; adopción en CI/PR = operator-gated. | operator-gated |
| 2 | Mitad Rust del WO-04 SIGUE uncommitted (17 archivos) en branch `fix/wo15-xinfo-shape`; producción corre SOLO la mitad TS — el tip 2 gwei hardcodeado sigue vivo en la imagen relays-client deployada. Board "APPLIED (post-merge)" ≠ live. Empaquetado/branch = operador (riesgo §36 de stranding). | operator-gated |
| 3 | WO-04-VERIFY §1.(3) overclaim: "única ocurrencia del repo" de `2_000_000_000` es FALSA — `sed-core/src/connectors/gas_oracle.rs:77/88`, `mempool_listener.rs:184` (todos `#[cfg(test)]`), `swap_encoder.rs:312` (amount_out_minimum), `discovery_workload.rs:725` (nanos). Sustancia intacta (0 en código vivo tocado), precisión del reporte a corregir. | agent-fixable |
| 4 | `priority_fee_gwei` sin cota superior (schema `minimum: 0` único): typo operador (p.ej. 2000 gwei) fluye al tip del bundle firmado en TESTNET (mainnet físicamente rehusado por M1, paper no emite). Camino de fondos pide check defensivo (maximum en schema + warn al boot). Baja severidad dadas las barreras. | agent-fixable |
| 5 | `database/migrations/MIGRATION_HISTORY.md` sin entrada 119 (DECLARADO R8 §5.2; 120 sí tiene). 1 línea cuando el operador rule. | agent-fixable |
| 6 | Comentario stale `trading-config.ts:672` (verifier F1, mitad TS) sigue sin corregir. | agent-fixable |

## 4. Restricciones del cross-examiner cumplidas

CERO git write · VPS solo lectura (psql SELECT, docker logs/inspect, curl
interno 127.0.0.1) · 0 requests a dominio público (presupuesto 0/5) · §32/§33
audit/read-only · sin tocar archivos de código.
