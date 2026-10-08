# ALERT-OUTCOMES-01 — la alerta era ciega a las tres series que existen para diagnosticar

**Tarea:** t104 · **run_id:** `arbx-entrega-20261007` · **permisos:** rama + PR SIN merge; prohibido mergear, desplegar o reiniciar servicios.
**`main` medido:** `git ls-remote origin refs/heads/main` → `fa6f5284cc5bd704cd05bf89d37b50c257adbc74` (**coincide con el SHA_BASE del briefing**).
**Alcance:** único path modificado `monitoring/alerts.rules.yml`. `backend/`, `.github/`, `docker/`, `frontend/` **sin tocar**.

---

## 0. Veredicto en una línea

**La alerta `SIM_FUNDER_SLOT_UNRESOLVED` enumeraba un solo valor exacto (`outcome="slot_unresolved"`) y quedaba CIEGA a `write_rejected`, `verify_mismatch` y `balance_unreadable` — las tres series que #850 introdujo justamente para diagnosticar la causa del fondeo.** Ahora las cubre con una **alternación anclada de valores completos**, validada con **promtool 2.55.1 real** (`SUCCESS: 38 rules found`) y con el **motor de Prometheus en vivo** (el matcher nuevo sigue disparando para `slot_unresolved`, y **no captura** los tres veredictos de cartucho).

## 1. El hecho medido

| qué | valor | comando |
|---|---|---|
| La alerta, antes | `expr: increase(arbx_sim_funding_total{outcome="slot_unresolved"}[30m]) > 5` | `monitoring/alerts.rules.yml:120` (antes del cambio) |
| Valores de `outcome` que EXISTEN hoy en el motor | **`slot_unresolved` = 1 · `rpc_err` = 1** | `curl -s --data-urlencode 'query=count by (outcome) (arbx_sim_funding_total)' http://127.0.0.1:9090/api/v1/query` → `{"status":"success",…"result":[…"outcome":"slot_unresolved"…"outcome":"rpc_err"…]}` |
| Los tres `funding_*` en `monitoring/` | **no aparecen** (solo en el comentario nuevo, ver §3.4) | `grep -rn 'funding_edge_negative\|…' monitoring/` |

**Una capacidad de diagnóstico que la capa de alertas no ve es una capacidad que nadie va a mirar.** #850 creó las tres series para poder decir *por qué* falla el fondeo; sin cobertura, el diagnóstico existe pero es invisible.

## 2. El cambio (M1)

`monitoring/alerts.rules.yml` — **1 archivo, +22/−3**. El matcher nuevo, literal de la línea 139:

```
        expr: increase(arbx_sim_funding_total{outcome=~"slot_unresolved|write_rejected|verify_mismatch|balance_unreadable"}[30m]) > 5
```

- La alerta **sigue llamándose igual** (`SIM_FUNDER_SLOT_UNRESOLVED`) para no romper silencios ni dashboards existentes, y **conserva** `for: 5m`, `severity: warning`, `service: sim-ctl`.
- `> 5` **sigue siendo por serie** (comparación vectorial en PromQL) ⇒ la alerta sigue diciendo **CUÁL** outcome sube, ahora mediante `{{ $labels.outcome }}` en `summary` y `description`.
- La `description` nombra las cuatro causas y de dónde vienen (`slot_unresolved` = el probe de balance-slot no resuelve el layout; `write_rejected`/`verify_mismatch`/`balance_unreadable` = el write de fondeo fue rechazado, la verificación posterior discrepó, o el balance no se pudo leer — #850).

## 3. M2 — El matcher es EXACTO, y está demostrado que no se traga los veredictos

### 3.1 Por qué una alternación anclada es exacta

PromQL `=~` está **completamente anclado** (motor RE2: el patrón debe cubrir el valor ENTERO). Por eso una alternación de valores literales completos **no puede** capturar ningún otro valor.

### 3.2 Demostración sobre el matcher extraído del archivo (no transcrito)

`matcher=slot_unresolved|write_rejected|verify_mismatch|balance_unreadable` · `alternativas=4 -> ['slot_unresolved','write_rejected','verify_mismatch','balance_unreadable']`

| valor | con el matcher EXACTO (anclado) | grupo |
|---|---|---|
| `slot_unresolved` · `write_rejected` · `verify_mismatch` · `balance_unreadable` | **MATCH** (4/4) | intencionados |
| `funding_edge_negative` · `funding_differential_within_band` · `funding_direction_unfavorable` | **no-match (0/3)** | **PROHIBIDOS** |
| `rpc_err` · `slot_unresolved_extra` · `xwrite_rejected` · `write_rejected_x` · `''` · `slot` · `SLOT_UNRESOLVED` | **no-match (0/7)** | ruido por anclaje |

### 3.3 Contraprueba: un matcher laxo SÍ los tragaría (el requisito no es vacuo)

| matcher laxo | captura sobre los tres prohibidos |
|---|---|
| `funding_.*` | **3/3** |
| `.*funding.*` | **3/3** |
| `funding` | 3/3 con `search` (0/3 con anclaje) |

Desde el **motor real**, los tres valores prohibidos devuelven vacío: `count(arbx_sim_funding_total{outcome=~"funding_edge_negative|funding_differential_within_band|funding_direction_unfavorable"})` → `{"status":"success","data":{"resultType":"vector","result":[]}}`.

**El matcher, verificado por sus partes:** `alternativas_que_son_veredicto_de_cartucho=0` · `alternativas_que_contienen_funding=0` · `alternativas_con_comodin_o_metacaracter=0` · `alternativas_exactas=SI` (todas `[a-z_]+`).

### 3.4 Instrumento débil vs. instrumento fuerte (declarado, no escondido)

`grep -n 'funding_edge_negative\|…' monitoring/alerts.rules.yml` → **2 líneas, RC=0**, y son **mi propio comentario** (L129-130) que nombra la trampa para documentarla. **Un grep de archivo matchea prosa: no prueba nada sobre el matcher.** Los instrumentos que sí deciden:

```
$ grep -n '^\s*expr:' monitoring/alerts.rules.yml | grep 'funding_edge_negative\|…'
(vacío)  RC_GREP_EXPR=1        <-- el matcher NO los contiene
$ grep -c 'alert:' monitoring/alerts.rules.yml
38      RC_ALERTAS=0           <-- CONTROL: el instrumento SI encuentra cosas
```

Sin ese control, un `0` con `RC=1` sería indistinguible de un instrumento roto.

## 4. M3 — La alerta existente NO se rompió: probado con el motor

Los dos matchers evaluados por **el mismo Prometheus en vivo**, contra las series reales:

```
VIEJO  count(increase(arbx_sim_funding_total{outcome="slot_unresolved"}[30m]) > 5)
       -> {"status":"success","data":{"resultType":"vector","result":[{"metric":{},"value":[1791420353.999,"1"]}]}}   rc=0
NUEVO  count(increase(arbx_sim_funding_total{outcome=~"slot_unresolved|write_rejected|verify_mismatch|balance_unreadable"}[30m]) > 5)
       -> {"status":"success","data":{"resultType":"vector","result":[{"metric":{},"value":[1791420353.986,"1"]}]}}   rc=0
```

**Ambos devuelven exactamente la misma serie (count = 1)**: el matcher nuevo sigue disparando para `slot_unresolved` **y no ensancha** el conjunto contra los datos que existen hoy. Eso prueba M3 *y* da la parte de M2 que el motor puede probar con datos vivos.

## 5. M6 — Validación: con qué herramienta, y con qué exit code

| entorno | `command -v promtool` | lectura |
|---|---|---|
| Git Bash (esta estación) | **RC=1**, sin ruta | **no está instalado** |
| WSL (sonda explícita) | **RC=0 pero SIN ruta impresa** | **medición INCONSISTENTE**: no la uso como evidencia de disponibilidad. La declaro en vez de leerla como "está". |
| **contenedor `arbitragex-v2-prometheus-1`** | `promtool, version 2.55.1 (branch: HEAD, revision: 6d7569113f1ca814f1e149f74176656540043b8d)`, **rc=0** | **la herramienta REAL, disponible** |

**Validación ejecutada con la herramienta real, sobre NUESTROS bytes, sin escribir nada en producción** (el archivo va por `stdin`, `/dev/stdin`, `docker exec -i`):

```
$ docker exec -i arbitragex-v2-prometheus-1 promtool check rules /dev/stdin < monitoring/alerts.rules.yml
Checking /dev/stdin
  SUCCESS: 38 rules found
cmd_ssh_exit=0
```

**38 reglas** — el mismo número que antes del cambio (el archivo sigue teniendo 38 alertas: no se agregó ni se quitó ninguna). Archivo local: **30.451 bytes, `CR=0` (LF puro)**.

**Dos trampas de instrumento que me mordieron acá y quedan declaradas:**
1. **`bash -s` y `docker exec -i` pelean por el MISMO stdin**: mi primer intento metió el *script* dentro de promtool y la salida fue `line 1: cannot unmarshal !!str 'echo "r...' into rulefmt.RuleGroups` con **`ssh_exit=1`**. Un archivo de reglas "inválido" que en realidad era mi propio script: sin el exit code, eso se leía como un error de YAML del archivo. Resuelto alimentando los bytes por un caño (`base64 -d | docker exec -i …`), sin compartir stdin.
2. Mi extractor tomó primero la línea `description` (que también contiene el matcher, con comillas escapadas) y falló con `AttributeError: 'NoneType'`. Corregido anclando en `l.strip().startswith('expr:')`.

## 6. M4/M5 — Alcance y entrega

- **Único path modificado: `monitoring/alerts.rules.yml`** (`+22/−3`, 1 archivo). Nada fuera de `monitoring/`; no hizo falta tocar nada más, así que no hubo que detenerse.
- **Rama + PR contra `main`, SIN merge.** **Cero deploy, cero reinicio de Prometheus/Alertmanager, cero cambios en producción**: el cambio es configuración versionada.
- Identidad del clon configurada por `git config --local` (`arbx-sre/sre@arbx.local`) antes del primer commit; push verificado **por el remoto**.

## 7. Declarado, NO hecho (para el operador)

1. **`rpc_err` ya existía en esta métrica y ya estaba fuera de esta alerta antes de hoy.** No lo agregué: la orden nombra tres valores y ampliar el alcance por mi cuenta sería una decisión de gobernanza, no una reparación. Queda **medido y nombrado** para que el operador decida.
2. **Las tres series nuevas todavía NO emiten** (el motor sólo ve `slot_unresolved` y `rpc_err` → #850 no está desplegada). Por eso su cobertura está probada por **semántica del matcher + validación de la regla con promtool + el motor en vivo sobre los valores que sí existen**, no por haber visto la alerta dispararse con datos reales de esas tres series. **Eso último es NO COMPUTADO y no se hereda**: cuando #850 se despliegue, la prueba de fuego es ver `outcome=write_rejected` (o las otras) en `arbx_sim_funding_total` y comprobar que la alerta lo toma — el matcher ya lo cubre, pero eso se **re-mide**, no se asume.
3. La `description` cita `arbx_sim_funding_total{outcome=~"…"}` con el matcher completo: si el matcher cambia, la prosa hay que actualizarla también (el comentario dejó escrito el porqué del anclaje para que el próximo autor no lo relaje).

## 8. Reproducción

```bash
# M6: disponibilidad y validación con la herramienta REAL (sin escribir en produccion)
command -v promtool                                   # Git Bash: RC=1
docker exec arbitragex-v2-prometheus-1 promtool --version        # rc=0, 2.55.1
cat monitoring/alerts.rules.yml | ssh arbx "docker exec -i arbitragex-v2-prometheus-1 promtool check rules /dev/stdin"
#   -> Checking /dev/stdin / SUCCESS: 38 rules found
# M2/M3: el matcher, por partes y con el motor vivo
grep -n '^\s*expr:' monitoring/alerts.rules.yml | grep 'funding_edge_negative\|funding_differential_within_band\|funding_direction_unfavorable'   # RC=1, vacio
grep -c 'alert:' monitoring/alerts.rules.yml          # 38  (control: el instrumento SI encuentra)
ssh arbx "curl -s --data-urlencode 'query=count by (outcome) (arbx_sim_funding_total)' http://127.0.0.1:9090/api/v1/query"
ssh arbx "curl -s --data-urlencode 'query=count(increase(arbx_sim_funding_total{outcome=~\"slot_unresolved|write_rejected|verify_mismatch|balance_unreadable\"}[30m]) > 5)' http://127.0.0.1:9090/api/v1/query"
```
