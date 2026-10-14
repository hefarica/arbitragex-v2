# G2-CHANNEL-01 — canal READ-ONLY del tally de `simulations`

**Estado: LOGRADO.** El canal existe, corre contra la base viva, y **devuelve filas**. Con eso `G2` deja de
ser NO COMPUTADO: pasa a ser **medible**, y su valor medido es **`simulations_passed = 0`** ⇒ **G2 = FAIL**.

| | Valor |
|---|---|
| Canal | `.github/workflows/g2-tally-readonly.yml` (nuevo) |
| PR | **#825** — https://github.com/hefarica/arbitragex-v2/pull/825 (OPEN, sin merge) |
| Run que lo prueba | **`37410022267`** → `completed/success`, 6 steps, `runner_id=1000095409` |
| Artefacto del run | `g2-tally-37410022267-1` (1323 b) |
| Mecanismo de acceso | `secrets.VPS_SSH_*` + `ssh` — **el mismo de `t37`**, no uno nuevo |

---

## 1. El defecto que esto cierra

`G2` (*"≥1 simulación passed"*) no estaba en FAIL: estaba **NO COMPUTADO**, porque el único canal
disponible estaba **MUDO**. Y se probó **con su control**, que es lo que lo vuelve concluyente:

| Consulta | Canal viejo (`sql_query`) |
|---|---|
| `SELECT 1 AS control;` | `{"ok":true,"data":"","rows_affected":0}` — **vacío** |
| `SELECT COUNT(*) FROM simulations WHERE passed;` | `{"ok":true,"data":"","rows_affected":0}` — **vacío** |

**`SELECT 1` SIEMPRE devuelve exactamente una fila.** Que el control también venga vacío **prueba que el
canal no contesta** — no que el valor sea cero. **Un payload vacío no es un 0.** Sin el control, "vacío" y
"cero" serían indistinguibles, y rellenar con `0` habría sido inventar un FAIL (o peor, un PASS).

---

## 2. El canal, y su control — evidencia cruda del run vivo

```
preflight OK: container arbitragex-v2-postgres-1 running=true
MEASURED control_select_1       -> 1
MEASURED g2_simulations_passed  -> 0
```

**Las dos filas importan, y juntas son la prueba:**

- `control_select_1 -> **1**` ⇒ **el canal devuelve filas**. Habla.
- `g2_simulations_passed -> **0**` ⇒ **y ese 0 es un cero REAL**, precisamente porque el control demuestra
  que el canal no está mudo. Esa es la distinción que el contrato exige probar, y acá está probada en la
  misma corrida.

**Consecuencia para la cadena de causalidad:** `G2` ya no es "no medible". Es **FAIL con artefacto
reproducible**, que es exactamente lo que §34.5 pide por gate. El cuello sigue siendo real (`sims_passed=0`),
pero ahora es **medido** y no **supuesto por silencio**.

**Nombres medidos, no supuestos:** el contenedor `arbitragex-v2-postgres-1` sale del propio repo
(`auto-deploy-vps.yml:177,181,187,313` → `docker exec arbitragex-v2-postgres-1 pg_isready -U postgres -d arbitragex`),
y la base/usuario (`arbitragex` / `postgres`) del mismo lugar. El preflight lo confirma en vivo:
`running=true`.

---

## 3. Cómo se garantiza READ-ONLY (cuatro capas independientes)

| # | Capa | Qué cierra |
|---|---|---|
| 1 | Input **`type: choice`** con lista cerrada (`g2_simulations_passed`, `control_select_1`) | **No acepta SQL libre.** `t54` ya descartó inyectar comandos por inputs; esto no reabre esa puerta. El input **nunca** se convierte en SQL. |
| 2 | El texto SQL sale de un **`case` fijo** en el workflow | El input sólo elige una **clave**; no puede aportar sintaxis. |
| 3 | `BEGIN TRANSACTION READ ONLY; … ROLLBACK;` | Una escritura **falla en el motor** ("cannot execute ... in a read-only transaction"). Además nada se confirma: la transacción se revierte siempre. |
| 4 | **Guard de inventario de verbos**, calcado del precedente del repo (`runtime-identity-probe.yml:160-175`) | Si el SQL ensamblado usa un token fuera de la whitelist, **aborta antes de tocar el host**. |

**El guard está probado en los dos sentidos**, no sólo declarado:

| SQL | Tokens fuera de whitelist | Resultado |
|---|---|---|
| `SELECT 1 AS control;` | **0** | pasa |
| `SELECT COUNT(*)::bigint AS simulations_passed FROM simulations WHERE passed;` | **0** | pasa |
| `DELETE FROM simulations WHERE passed;` (control negativo) | **`DELETE`** | **RECHAZADO** |

En el run real, el propio step lo imprime: `read-only guard OK: every assembled statement is a subset of the
whitelist`.

**Y si no devuelve fila, FALLA** con `::error::MUTE CHANNEL … an empty payload is NOT a zero`. El canal no
puede volver a mentir por silencio: la ausencia de fila es un error, no un cero.

---

## 4. Por qué el api-server no podía dar esto

`git grep -c 'FROM simulations' origin/main -- 'backend/api-server/'` → **0 coincidencias**. La tabla está
poblada y **ninguna ruta del api-server la lee**: no hay nada que exponer sin escribir código nuevo en
`backend/`, que está **fuera del alcance** de esta tarea (in scope: `.github/workflows/`, `docs/release/`).
Además, exponerlo cambiaría el **producto servido** — más superficie que un probe read-only. Se eligió el
canal de Actions, que es el camino que `t37` ya había probado contra el VPS.

### El "~640k" — NO COMPUTADO

La cifra de ~640k filas viene **citada** por `t54` y por la orden, no medida por mí. **Mi canal cuenta
`WHERE passed`, y eso es lo único que midió**: no produje un `COUNT(*)` de la tabla completa, así que
**la población total NO está re-medida en esta tarea**. No localicé el artefacto original del ~640k entre
los 90 archivos de `docs/`+`audits/` que contienen "640". Se declara **NO COMPUTADO** en vez de repetir una
cifra sin fuente propia. (`t54` ya había establecido que la tabla está poblada; lo que faltaba no era el
tamaño, era el canal.)

---

## 5. La iteración: el primer run falló, y por qué

El canal se probó a sí mismo en el PR (trigger `pull_request: paths: ['.github/workflows/g2-tally-readonly.yml']`,
porque un `workflow_dispatch` sólo es despachable si el archivo ya está en la rama por defecto — y este PR
**no se mergea**). El primer run (`37409619428`) **falló**, y los dos defectos eran míos:

1. **La clave SSH quedaba en `0644`** → `Load key: bad permissions` / `Permission denied (publickey)`.
   Faltaba `chmod 600`. **Arreglado** (verificado en el YAML: `chmod 600 ~/.ssh/deploy_key` presente).
2. **El diagnóstico mentía**: el mensaje decía *"container … not found on the host"* cuando lo que había
   fallado era el **ssh**. Un mensaje así manda al próximo operador a buscar el contenedor equivocado, y el
   contenedor estaba perfecto. **Arreglado con un discriminador de causa**: `ssh` usa **rc=255** para sus
   propios errores, y cualquier otro rc viene de `docker inspect`; ahora los dos casos se reportan distinto.

El segundo run (`37410022267`) dio **success** con las cinco filas de arriba.

**Nota de infraestructura, medida y no imputable al canal:** ese segundo run quedó **~16 minutos en
`queued` con `runner_id=0` y `steps=0`** (clase A: runner nunca adquirido), con **39 runs en cola** en el
repo en ese momento. Adquirió runner después y terminó bien.

---

## 6. Declaraciones exigidas

- **NO mueve P/N: sigue `0/115`.** El canal es de lectura; no cambia el SSOT ni el estado de ejecución.
- **No modifica datos NI esquema.** No hay DDL ni DML en ninguna ruta del workflow: las únicas sentencias
  posibles son `SELECT`, y toda la sesión corre en `BEGIN TRANSACTION READ ONLY` seguida de `ROLLBACK`.
- **Sin SQL libre por inputs de un workflow ajeno:** el input es `choice` cerrado y el SQL sale de un `case`
  del propio archivo. No se tocó ningún workflow de terceros (a diferencia del camino que `t54` descartó).
- **Sin firma, sin broadcast, sin capital.** `ARBX_TRADE_MODE=paper` intacto.
- **Sin DML accidental en la corrida:** el tally corrió dentro de `READ ONLY` + `ROLLBACK`; el preflight fue
  un `docker inspect` (sólo lectura), y el único otro comando es `docker ps`.
- No se mergea.

---

## 7. Evidencia reproducible

```
gh run view 37410022267 --repo hefarica/arbitragex-v2 --log
  -> read-only guard OK: every assembled statement is a subset of the whitelist
     control_select_1      SELECT 1 AS control;
     g2_simulations_passed SELECT COUNT(*)::bigint AS simulations_passed FROM simulations WHERE passed;
     preflight OK: container arbitragex-v2-postgres-1 running=true
     MEASURED control_select_1       -> 1
     MEASURED g2_simulations_passed  -> 0
gh api repos/hefarica/arbitragex-v2/actions/runs/37410022267/jobs   -> 6 steps, completed/success
gh api repos/hefarica/arbitragex-v2/actions/runs/37410022267/artifacts -> g2-tally-37410022267-1 (1323 b)

# el canal viejo, con su control (el defecto que esto cierra):
sql_query SELECT 1 AS control;                          -> {"ok":true,"data":"","rows_affected":0}
sql_query SELECT COUNT(*) FROM simulations WHERE passed; -> {"ok":true,"data":"","rows_affected":0}

git grep -c 'FROM simulations' origin/main -- 'backend/api-server/'   -> 0
```
