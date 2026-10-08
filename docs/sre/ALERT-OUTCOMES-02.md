# ALERT-OUTCOMES-02 — `rpc_err` es el otro fallo del ARNÉS, y excluirlo era una inconsistencia

**Tarea:** t105 · **run_id:** `arbx-entrega-20261007` · **permisos:** rama + PR SIN merge; prohibido mergear, desplegar o reiniciar servicios.
**Base:** rama `sre/alert-outcomes-01` @ `7084b55a` (PR **#853**, t104) — **apilada a propósito**, ver §7.
**`main` medido:** `fa6f5284cc5bd704cd05bf89d37b50c257adbc74` (coincide con el SHA_BASE del briefing).
**Alcance:** `monitoring/alerts.rules.yml` + este documento. `backend/`, `.github/`, `docker/`, `frontend/` **sin tocar**.

---

## 0. Veredicto en una línea

**`rpc_err` queda cubierto por la misma alerta, con el matcher exacto de 5 valores: `5/5` intencionados, `0/3` veredictos de cartucho, `0/7` ruido de anclaje — y la condición de alerta sigue devolviendo LA MISMA serie que antes** (probado con el motor vivo, no afirmado).

## 1. N1 — Por qué entra, y por qué la razón es de CLASE (texto de la justificación)

`slot_unresolved` y `rpc_err` son **los dos fallos del ARNÉS de la misma función**:

| outcome | qué dice | naturaleza |
|---|---|---|
| `slot_unresolved` | el centinela no se reprodujo en ningún slot candidato | **fallo del arnés** |
| `rpc_err` | una llamada RPC del camino de fondeo falló | **fallo del arnés** |
| `write_rejected` · `verify_mismatch` · `balance_unreadable` (#850) | el write de fondeo fue rechazado / la verificación discrepó / el balance no se pudo leer | **fallo del arnés** |
| ~~`funding_edge_negative` · `funding_differential_within_band` · `funding_direction_unfavorable`~~ | veredictos de cartuchos de searcher-rs | **veredicto de MERCADO — NO entran** |

**Alertar por el primero mientras se excluye al segundo no es un límite de alcance: es una INCONSISTENCIA.** El escenario que nos dejaría ciegos es precisamente que el **modo de fallo se CORRA** de `slot_unresolved` a `rpc_err` — y eso es exactamente lo observable si el proveedor RPC degrada o aplica rate-limit. **Medido (t104): `rpc_err` ya existía en `arbx_sim_funding_total` y ya estaba fuera de esta alerta ANTES de #850.** Con `sim_signer_funding_slot_unresolved` al **98,740 %** de la cadena post-deploy (medición de t93), esta métrica es hoy **el diagnóstico primario del pipeline**: dejarla parcialmente ciega es dejar ciego el instrumento que más vamos a mirar.

## 2. El cambio

`monitoring/alerts.rules.yml`, **+20/−6** (1 archivo):

```diff
-        expr: increase(arbx_sim_funding_total{outcome=~"slot_unresolved|write_rejected|verify_mismatch|balance_unreadable"}[30m]) > 5
+        expr: increase(arbx_sim_funding_total{outcome=~"slot_unresolved|rpc_err|write_rejected|verify_mismatch|balance_unreadable"}[30m]) > 5
```

`slot_unresolved` y `write_rejected` los precedentes se conservan; `severity`, `for: 5m`, el nombre y el umbral `> 5` **intactos**. El bloque `ALERT-OUTCOMES-01` que decía *"Deliberately NOT included: `rpc_err`"* fue **reemplazado** por el bloque `ALERT-OUTCOMES-02` con la justificación de clase de §1 (dejarlo habría contradicho el matcher). La `description` ahora nombra las cinco causas y aclara que todas son fallos del arnés, ninguna un veredicto de mercado.

## 3. N2 — El matcher sigue siendo EXACTO, y está demostrado

Matcher extraído del archivo (no transcrito): `slot_unresolved|rpc_err|write_rejected|verify_mismatch|balance_unreadable` · `alternativas=5`.

- `todas_literales_[a-z_]+=True` · **`contienen_funding=0`** · **`con_metacaracter=0`**.

| valor | anclado (PromQL `=~` = full-anchor RE2) | grupo |
|---|---|---|
| `slot_unresolved` · **`rpc_err`** · `write_rejected` · `verify_mismatch` · `balance_unreadable` | **MATCH (5/5)** | intencionados |
| `funding_edge_negative` · `funding_differential_within_band` · `funding_direction_unfavorable` | **no-match (0/3)** | PROHIBIDOS |
| `slot_unresolved_extra` · `xwrite_rejected` · `write_rejected_x` · `SLOT_UNRESOLVED` · `''` · `slot` · **`rpc_err_x`** | **no-match (0/7)** | ruido de anclaje |

**Contraprueba (el requisito no es vacuo):** `funding_.*` captura **3/3** prohibidos (y `.*funding.*` también).

## 4. N3 — No se ensancha la condición, y la alerta no se rompe: medido con el motor

**Universo vivo** (`count by (outcome) (arbx_sim_funding_total)`): `slot_unresolved`=1 · `rpc_err`=1.

**La CONDICIÓN de alerta, tres versiones del matcher, contra el motor real** — todas devuelven **count = 1**, es decir **la MISMA serie**:

| versión | consulta | resultado |
|---|---|---|
| `main` (1 valor) | `count(increase(arbx_sim_funding_total{outcome="slot_unresolved"}[30m]) > 5)` | `…"value":[1791420594.243,"1"]` |
| #853 (4 valores) | `…{outcome=~"slot_unresolved\|write_rejected\|verify_mismatch\|balance_unreadable"}…` | `…"value":[1791420594.251,"1"]` |
| **t105 (5 valores)** | `…{outcome=~"slot_unresolved\|rpc_err\|write_rejected\|verify_mismatch\|balance_unreadable"}…` | `…"value":[1791420594.258,"1"]` |

Y la prueba más fina — el **detalle** de la serie que dispara, con el matcher de 5 valores:

```
increase(arbx_sim_funding_total{outcome=~"rpc_err|slot_unresolved"}[30m]) > 5
  -> {"metric":{"outcome":"slot_unresolved","instance":"sim-ctl:3003","job":"sim-ctl","service":"sim-ctl"},"value":[…,"7246.3865546218485"]}
```

**Sólo `slot_unresolved` (7246,39 en 30m) satisface la alerta**: la alerta **ya estaba disparando** antes de este cambio y **sigue disparando por la misma serie**. No se rompió nada.

**Dónde SÍ hay ensanchamiento, declarado y por diseño:** el **selector crudo** pasa de **1** a **2** series (`count(arbx_sim_funding_total{outcome=~"<4 valores>"})` = 1 → `count(…"<5 valores>")` = 2). Eso *es* el objetivo de la tarea: la alerta ahora **vigila** `rpc_err` además de `slot_unresolved`.

**¿Arma una alerta viva hoy?** `increase(arbx_sim_funding_total{outcome="rpc_err"}[30m])` = **0** ⇒ `rpc_err` existe pero **no se ha movido** en 30m: la cobertura nueva queda **armada y en silencio**, sin disparar nada por este cambio.

## 5. N4 — Validación con la herramienta REAL sobre NUESTROS bytes

| entorno | `command -v promtool` |
|---|---|
| Git Bash (esta estación) | **RC=1**, sin ruta ⇒ no instalado |
| contenedor `arbitragex-v2-prometheus-1` | **`promtool, version 2.55.1 …`**, **rc=0** ⇒ la herramienta real está disponible y **la validación fue posible** |

Corrido sobre **nuestros bytes**, por `/dev/stdin`, **sin escribir nada en producción** (y sin compartir el stdin de `bash -s`, que fue la trampa de t104: se alimenta por caño `base64 -d | docker exec -i`):

```
Checking /dev/stdin
  SUCCESS: 38 rules found
RC_PROMTOOL_CHECK=0
```

**38 reglas = el mismo inventario que en `main` y que en #853**: no se agregó ni se quitó ninguna alerta.

## 6. N5 — Los `verify` no grepean el archivo: grepean el matcher, con CONTROL

- `grep -n '^\s*expr:' monitoring/alerts.rules.yml` → **36 líneas `expr:`**, con el matcher nuevo en **L150**; `RC_GREP_EXPR=0`.
- **CONTROL de inventario**: `grep -c 'alert:' monitoring/alerts.rules.yml` → **38**, `RC_CONTROL=0`. **Sin este control, un vacío con RC=1 sería indistinguible de un instrumento roto.**
- Contraste que justifica la regla: el grep sobre el **archivo entero** para `funding_edge_negative` devuelve **1** (`RC_PROSA=0`) y es **prosa** — el comentario de t104 que nombra la trampa. **Un grep de archivo matchea prosa y no prueba nada sobre el matcher.**
- Los cinco valores aparecen **2 veces** (`RC_CINCO=0`) y son exactamente las dos líneas del cambio: el `expr:` y la `description`.

## 7. N6 — Alcance y entrega (stacked, declarado)

**Rama:** `sre/alert-outcomes-02`, **apilada sobre `sre/alert-outcomes-01` @ `7084b55a`** (PR **#853**, t104).

**Por qué apilada y no desde `main`:** los dos PRs tocan **la misma línea**. Sacar una rama limpia desde `main` habría producido **dos versiones rivales del mismo matcher** (4 valores vs 5). Apilado, el diff de este PR es **exactamente el incremento ordenado** (+`rpc_err`), y el resultado es el mismo se aterrice en cualquier orden: **aterrizar #854 entrega ambos cambios**; aterrizar #853 y después #854 también. La unión es **intencional**, no un apilamiento accidental (R13.4: la base se documenta en el cuerpo del PR).

**Sin merge, sin deploy, sin reiniciar Prometheus/Alertmanager, sin tocar producción**: es configuración versionada. Sin firma, sin capital.

## 8. Declarado — lo que NO está computado

1. **Las tres series de #850 todavía NO emiten** (el motor sólo ve `slot_unresolved` y `rpc_err`) ⇒ su cobertura sigue probada por **semántica del matcher + promtool + el motor sobre los valores vivos**, no por haberlas visto disparar. Se **re-mide** cuando #850 se despliegue; no se hereda.
2. **La cobertura de `rpc_err` está armada pero no ejercitada**: `increase(rpc_err[30m]) = 0` ⇒ no hay un aumento real que haya disparado la regla. Lo que está probado es que la serie **está vigilada** y que el umbral por serie la tomaría si subiera.
3. **`slot_unresolved` viene en 7246,39/30m**: la alerta **está disparando hoy** por esa serie. Este cambio **no la silencia ni la intensifica** (la condición devuelve la misma serie), pero el operador debe saber que el rojo ya está encendido y por qué.

## 9. Reproducción

```bash
# N5: el matcher (no el archivo) + control de inventario
grep -n '^\s*expr:' monitoring/alerts.rules.yml            # RC=0, matcher en L150
grep -c 'alert:' monitoring/alerts.rules.yml                # 38  (CONTROL)
# N4: validacion con la herramienta REAL, sobre nuestros bytes, sin escribir en produccion
cat monitoring/alerts.rules.yml | ssh arbx "docker exec -i arbitragex-v2-prometheus-1 promtool check rules /dev/stdin"
#   -> Checking /dev/stdin / SUCCESS: 38 rules found
# N3: la condicion, tres versiones del matcher, contra el motor vivo
ssh arbx "curl -s --data-urlencode 'query=count by (outcome) (arbx_sim_funding_total)' http://127.0.0.1:9090/api/v1/query"
ssh arbx "curl -s --data-urlencode 'query=increase(arbx_sim_funding_total{outcome=\"rpc_err\"}[30m])' http://127.0.0.1:9090/api/v1/query"
ssh arbx "curl -s --data-urlencode 'query=increase(arbx_sim_funding_total{outcome=~\"rpc_err|slot_unresolved\"}[30m]) > 5' http://127.0.0.1:9090/api/v1/query"
```
