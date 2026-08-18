# AUDITORÍA: Excel → VPS Migration — Discrepancias y Estado

> Auditado: 2026-08-15 · SSOT: `Downloads/ArbitrageX_Unified_Config.xlsm` (123KB, 14-ago) · Macro: `ArbxEnvDeploy.bas`

## Arquitectura del pipeline de configuración

```
Excel SSOT (15 hojas)
  ├── ".env Production" (191 vars) ←─ MACRO VBA lee/escribe aquí
  ├── "Secrets Deploy" (10 vars)    ←─ Documentación de referencia
  ├── "RPC Providers" (109 rows)   ←─ gen_rpc_env_from_xlsx.py lee aquí
  └── "_RED_lookup" (110 rows)     ←─ gen_rpc_env_from_xlsx.py lee aquí

MACRO VBA (RunFullSyncCycle):
  1. PULL: SSH al VPS → lee .env.example → muestra qué falta
  2. REVIEW: operador llena los gaps en ".env Production"
  3. PUSH: ExportCore() escribe fragment → PowerShell GUI → SSH → upsert VPS .env

SCRIPT PYTHON (gen_rpc_env_from_xlsx.py):
  Lee "RPC Providers" + "_RED_lookup" → genera RPC_HTTP_*/RPC_WS_* lines
  → Output: rpc_env_generated.env (gitignored)
  → NO se ejecuta automáticamente en el ciclo de la macro
```

## DISCREPANCIAS ENCONTRADAS (5)

### D1 — DUPLICACIÓN entre hojas (⚠️ RIESGO MEDIO)

**Qué:** 8 vars de pool_enum/subgraph aparecen en AMBAS hojas (`.env Production` y `Secrets Deploy`) con los mismos valores.

**Riesgo:** Si el operador edita solo una hoja, el deploy lleva el valor de `.env Production` (que es la que la macro lee), pero la hoja `Secrets Deploy` queda desactualizada → confusión en futuras ediciones.

**Veredicto:** La hoja `Secrets Deploy` es **documentación/referencia**, no fuente. La macro SOLO lee `.env Production` (`ENV_SHEET = ".env Production"` en `ArbxEnvDeploy.bas:21`). No es un bug, pero hay que etiquetar la hoja como "REFERENCIA - NO EDITAR AQUÍ".

---

### D2 — RPC lines NO se actualizan con la macro (🔴 CRÍTICO)

**Qué:** La macro VBA solo lee/escribe `.env Production` (191 vars). Las líneas `RPC_HTTP_*` y `RPC_WS_*` NO están en esa hoja — las genera el script Python separado.

**Evidencia:**
- `ArbxEnvDeploy.bas:21`: `Private Const ENV_SHEET As String = ".env Production"`
- `ExportCore()` (línea 39-59): itera SOLO las filas de esa hoja
- `.env Production` no contiene `RPC_HTTP_1` ni `RPC_WS_1`

**Consecuencia:** Si el operador edita los RPCs en "RPC Providers" y hace click en "Deploy Secrets", los RPC lines **NO se actualizan** en el VPS. Quedan los del último deploy manual del gen script.

**Fix recomendado:** La macro debería ejecutar el gen script antes del PUSH, o el gen script debería escribir sus líneas DENTRO de `.env Production`.

---

### D3 — 3 providers sin usar (ℹ️ INFO)

Ankr, Infura, QuickNode están en `_RED_lookup` pero NO en `RPC Providers`. No es bug — son providers disponibles si el operador los agrega.

---

### D4 — Alchemy endpoint público sin key (🔴 BLOQUEA A2)

**Qué:** El VPS tiene `alchemy=https://eth-mainnet.g.alchemy.com/public` en `RPC_HTTP_1`. Esta URL NO tiene formato `/v2/<KEY>`.

**Consecuencia:** `extract_alchemy_key_from_rpc_env()` no puede extraer una key → `price_alchemy_hits = 0` → safety score < 50 → 100% rechazadas.

**Fix:** Cambiar en la hoja `RPC Providers` la URL de Alchemy de `/public` a `/v2/TU_KEY` (requiere cuenta en dashboard.alchemy.com). Este es exactamente el bloqueo A2 de la escalada.

---

### D5 — DOS versiones del Excel (⚠️ RIESGO BAJO)

- `Downloads/`: 123KB, 14-ago — **NUEVA** (tiene hoja `Secrets Deploy`)
- `Documents/`: 105KB, 7-ago — **VIEJA** (sin `Secrets Deploy`)

El gen script usa Downloads por default. Si alguien edita Documents, esos cambios no llegan.

**Fix:** Eliminar o archivar la versión de Documents.

---

## VERIFICACIÓN VIVO: Excel → VPS

| Concepto | Excel dice | VPS tiene | ¿Coincide? |
|---|---|---|---|
| RPC_HTTP_1 (10 providers) | gen script produce 10 | 10 en .env | ✅ |
| RPC_WS_1 (4 providers) | gen script produce 4 | 4 en .env | ✅ |
| ALLOWED_ORIGINS | `.env Production` col A/B | 4 origins en VPS | ✅ |
| Pool enum vars (8) | `.env Production` + `Secrets Deploy` | 8 en VPS | ✅ |
| Tokens/secrets | `.env Production` (191 vars) | deployado via macro | ✅ |
| Alchemy key | `/public` (sin key) | `/public` en VPS | ✅ (pero ambos sin key) |

## RECOMENDACIONES (en orden de impacto)

1. **A2/CRÍTICO:** Cambiar Alchemy URL en `RPC Providers` de `/public` a `/v2/<KEY>` → desbloquea price oracle → safety score sube → primera oportunidad aceptada
2. **ALTO:** La macro debería invocar `gen_rpc_env_from_xlsx.py` antes del PUSH (o el gen script debería escribir dentro de `.env Production`) para que las RPC lines se actualicen en el mismo click
3. **MEDIO:** Etiquetar la hoja `Secrets Deploy` como "REFERENCIA" para evitar confusión
4. **BAJO:** Eliminar la copia vieja en `Documents/`
