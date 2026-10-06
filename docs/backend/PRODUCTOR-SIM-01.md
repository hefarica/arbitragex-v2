# PRODUCTOR-SIM-01 — la cadena `sim-ctl` → `simulations`, y por qué NO puedo nombrar el corte

**Orden:** t66 · **Perfil:** Backend · **Intento:** 2 · `dc97a673-2336-44be-a0f6-d2d8aaf5be0c`
**Base:** `main` = `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e` · **Modo:** paper. Sin firma, sin broadcast, sin capital, sin deploy.
**Código modificado: NINGUNO.** Este documento es el único artefacto. **No se escribió ni una fila en `simulations`.**

> ## ⚠️ VEREDICTO: BLOQUEADO. NO puedo cerrar el corte, y no lo voy a fingir.
>
> El canal de medición está **denegado** en este entorno: `ssh` no ejecuta (exit **255** incluso con `ssh -V`, que sólo imprime la versión) y el canal SQL está **MUDO** (`SELECT 1` —el control que la propia orden exige— devuelve **sin fila**). Sin esos dos canales **no puedo** leer los logs, **no puedo** consultar el esquema vivo, y **no puedo** medir el `count(*)` post-fix.
>
> Quedan **dos mecanismos mutuamente excluyentes** que sólo los logs separan (§4). Nombrar uno como *el* corte sin esa evidencia sería seguridad fingida, que es lo único peor que no encontrarlo.

---

## 1. LA CADENA, con `archivo:línea`

| # | Eslabón | Locus |
|---|---|---|
| 1 | Lee `arbx:opps:validated` con `XREADGROUP GROUP sim-ctl-g0 <consumer> COUNT 8 BLOCK 2000 STREAMS ... >` | `consumer.rs:157-170` |
| 2 | Despacha cada entrada: `self.process_message(id, fields).await.ok();` | **`consumer.rs:182`** |
| 3 | Elige backend: `Some(b2c)` → `simulate_b2c()`; `None` → `self.backend.simulate()` (anvil default) | `consumer.rs:408-441` |
| 4 | **Ambas ramas convergen** y producen un único `sim` | `consumer.rs:441` |
| 5 | **Persiste**: `insert_simulation(&self.pool, &sim)` | **`consumer.rs:451`** |
| 6 | `INSERT INTO simulations (...) VALUES (...) ON CONFLICT (opportunity_id) WHERE simulator = 'revm' DO NOTHING` | **`persistence.rs:28-36`** (el `INSERT` se abre en **L30**) |
| 7 | `UPDATE opportunities SET status=… WHERE id=$1 AND status IN ('validated','scored','detected')` | `persistence.rs:94-109` |
| 8 | XACK **sólo tras persistir** | `consumer.rs:477-481` |

**Tabla de origen:** `database/migrations/004_simulations.sql:4-18` (13 columnas — coincide con lo medido).

---

## 2. RESPUESTA A LA PREGUNTA CENTRAL: `insert_simulation` **SÍ ES LLAMADA**

**Es llamada.** No por lectura: por **estructura del flujo**, que es verificable sin DB.

`insert_simulation` aparece en **exactamente dos** lugares en todo `backend/sim-ctl/src/`:
- `consumer.rs:28` — el `use`.
- **`consumer.rs:451` — la ÚNICA llamada.**

Y `consumer.rs:408-441` garantiza que **las dos ramas** (B2c y legacy/anvil) desembocan en la variable `sim`, que L451 consume. Es decir:

```
Some(b2c) → simulate_b2c(...) ─┐
                               ├─→ sim ─→ insert_simulation (L451) ─→ XACK (L479)
None      → backend.simulate() ┘
```

**Conclusión: el defecto NO es "nunca se llama".** Descartar esa mitad es metadato real: el fix NO es "conectar una llamada que falta", porque la llamada está y es única y común a los dos caminos.

> **Precisión sobre el alcance de esta prueba:** que L451 esté en el camino común prueba que **el código de `main` llama**. NO prueba que **el binario desplegado** sea este código. Sin acceso al contenedor no puedo excluir que el runtime corra una revisión anterior a SIMWIRE-02c. Queda declarado en §5.

---

## 3. LA PREGUNTA DEL `ON CONFLICT` — respondida, y con un filo

**¿Tiene el índice parcial que la cláusula exige?** **Sí, está DEFINIDO, y coincide exactamente:**

`database/migrations/113_simulations_revm_idempotency.sql:27-29`
```sql
CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS simulations_revm_idempotency_uq
  ON simulations (opportunity_id)
  WHERE simulator = 'revm';
```

El predicado del árbitro (`WHERE simulator = 'revm'`) es **idéntico** al del índice ⇒ la inferencia resuelve. **Por definición, la cláusula es válida.**

**PERO el cómo lo crea es el defecto latente, y el propio repo ya lo sabe:**

1. `CREATE UNIQUE INDEX CONCURRENTLY` **no puede correr dentro de una transacción** y, si aborta a mitad, **deja el índice INVALID**.
2. `IF NOT EXISTS` convierte **todo reintento en un no-op**: un índice INVALID **nunca se repara solo**. El comentario de la migración lo dice textual — *"If it ever fails midway it leaves an INVALID index: recovery is … documented, **not automated**"* (`113:22-26`).
3. PostgreSQL **excluye los índices INVALID** de la inferencia de `ON CONFLICT` ⇒ el `INSERT` no devuelve 0 filas: **falla** con `42P10` (*"there is no unique or exclusion constraint matching the ON CONFLICT specification"*).

**El propio test lo documenta como guarda** (`tests/simwire02c_redelivery_idempotency.rs:14-16`):
> *"If migration 113 is NOT applied, PG rejects the ON CONFLICT target ("no unique or exclusion constraint matching") and this test FAILS — that is the intended loud guard, not a flake."*

⇒ **Lo que la orden pedía distinguir, resuelto en abstracto:** un `ON CONFLICT` sin índice compatible **falla con error, no devuelve 0 filas**. Si éste es el mecanismo, `insert_simulation` **devuelve `Err`** (no `Ok(false)`).

---

## 4. EL CORTE: TRES MECANISMOS, DOS EXCLUYENTES, Y SÓLO LOS LOGS LOS SEPARAN

Los tres son **mutuamente distinguibles por una línea de log**, y ninguno lo puedo leer (§6).

### (A) `42P10` — índice parcial ausente o INVALID
`persistence.rs:35` falla → `Err` → `consumer.rs:453-456`:
```rust
Err(e) => {
    error!(event = "sim_consumer.persist_err", id=%id, error=%e);
    return Ok(());          // <-- NO hace XACK (return antes de L479)
}
```
Firma esperada: `sim_consumer.persist_err` con *"no unique or exclusion constraint matching"*.

### (B) `23503` — violación de FK, la entrada es un "ghost"
`simulations.opportunity_id … REFERENCES opportunities(id) **ON DELETE CASCADE**` (`004:6`) y **el cron de retención borró 13.7M filas de `opportunities`** (`116:3-5`). Si la fila de `opportunities` ya no existe, el `INSERT` viola la FK.
La guarda está en `consumer.rs:365-380` (`SELECT 1 FROM opportunities WHERE id = $1` → `GhostVerdict::RowMissing` → `"opportunity_row_missing"`), y **el camino de ghost ACKea** (`consumer.rs:753`: *"the simulations FK can never satisfy, PEL retry is futile, dead-letter is correct"*).

### (C) El binario desplegado no es este código
El runtime podría ser anterior a SIMWIRE-02c.

### EL DISCRIMINADOR QUE LOS SEPARA — y por qué (A) y (B) son **excluyentes**
**`pending`.** La orden midió **`pending 0`**. El comentario de `consumer.rs:443` promete *"if it fails, do NOT ack — retry on next iteration"*, y el código cumple: en `Err` **retorna antes del XACK de L479**. Por lo tanto:

- Si fuera **(A)** → cada persist falla → **no hay XACK** → el PEL crece → **`pending` > 0**. **Contradice `pending 0`.**
- Si fuera **(B)** → el camino de ghost **ACKea** → `pending 0` y **0 filas**. **Compatible con todo lo medido.**

⇒ **La evidencia medida apunta a (B), no a (A).** Pero **`pending 0` es de la orden, no mío** (mi canal está mudo, §6): lo tomo como dato de entrada y lo marco como tal, sin apuntalarlo con una medición propia que no puedo hacer.

**Corolario incómodo y material:** si es (B), el "0 filas" **NO es un fallo de `insert_simulation`** sino la **consecuencia honesta** de que las oportunidades ya no existen. Y entonces el defecto real está **aguas arriba**, en el productor/retención — **fuera de mi alcance** (`backend/searcher-rs/`, `scripts/`).

---

## 5. ESTADO DE LOS `Verify` DEL CONTRATO — 1 de 4 ejecutable

| # | Comando | Estado |
|---|---|---|
| 1 | `cargo test -p sim-ctl --no-fail-fast` | **EJECUTABLE** — pendiente de corrida en WSL |
| 2 | `docker exec … psql … "SELECT count(*) FROM simulations"` | **BLOQUEADO** — `ssh` no ejecuta (exit 255) |
| 3 | `docker exec … redis-cli XINFO GROUPS arbx:opps:validated` | **BLOQUEADO** — idem |
| 4 | `docker logs … \| grep -iE "persist\|insert\|sim\.stored\|sim\.skip\|db_error"` | **BLOQUEADO** — idem, **y es justo el que decide (A) vs (B)** |

**El comando que resolvería el caso es el #4, y es el que no puedo correr.**

**Sobre el test `SIMWIRE-02c` (aceptación 4):** existe (`tests/simwire02c_redelivery_idempotency.rs`, 8294 B). Su cabecera declara `:10-12`: *"Runs in the CI integration job (live PG + migrations 112/113 applied by `automation/scripts/migrate.sh` before `cargo test`). **Outside CI without `DATABASE_URL` it skips loudly — fail-honest, never fabricated**"*. Sin `DATABASE_URL` **se saltea**, así que **su salida NO puede probar ni refutar (A)**. Lo declaro en vez de presentar un skip como evidencia.

---

## 6. POR QUÉ NO PUDE MEDIR — con la prueba, no la excusa

| Canal | Prueba de que está caído |
|---|---|
| `ssh` | `ssh -V` —que sólo imprime la versión— devuelve **exit 255** con **0 bytes** de salida. Un binario que no puede ni reportar su versión no establece sesión. Descartado como problema de red: **`Test-NetConnection 195.201.235.70:22` → `TcpTestSucceeded = True`** y hay salida a internet (`github.com:443` → True). El alias `arbx` existe y su clave existe (432 B). |
| SQL | `sql_query 'SELECT 1 AS control'` → `{ok:true, data:"", rows_affected:0}`. **`SELECT 1` DEBE devolver una fila.** No la devuelve ⇒ **canal MUDO**. `sql_tables` → `count: 0`. |

**Por la regla de la propia orden — *"Un 0 con canal mudo NO es un 0"* — no reporto ninguna medición de base de datos.** No medí el `count(*)` post-fix porque **no hay post-fix que medir** y **no hay canal**.

---

## 7. LO QUE **NO** HICE, por la línea roja

- **NO escribí ninguna fila en `simulations`.** Ni estimada, ni de relleno, ni "temporal". Cero `INSERT`, cero puente, cero backfill.
- **NO fabriqué el `profit estimado`** del paso 2.2 del operador. Cerrar el indicador mientras el defecto sigue vivo es el modo de fallo que esta campaña persigue; hacerlo yo sería peor que no actuar.
- **NO modifiqué código** en `backend/sim-ctl/`: sin poder verificar el mecanismo ni correr un test contra PG, un "fix" sería una hipótesis con formato de parche. **`cargo test` no puede distinguir (A) de (B) sin `DATABASE_URL`.**
- **NO toqué sizing, límites de riesgo, ni nada fuera de `docs/backend/`.**

---

## 8. LO QUE HACE FALTA PARA CERRAR ESTO (concreto, ejecutable por quien tenga canal)

1. **Correr el Verify #4** (`docker logs … | grep -iE "persist|insert|db_error"`). Una línea decide (A) vs (B):
   - aparece `sim_consumer.persist_err` + *"no unique or exclusion constraint"* ⇒ **(A)**.
   - aparecen ghosts / `opportunity_row_missing` y **ningún** `persist_err` ⇒ **(B)**.
2. **Preguntar al esquema vivo** por el índice:
   `SELECT indexrelid::regclass, indisvalid FROM pg_index WHERE indexrelid = 'simulations_revm_idempotency_uq'::regclass;`
   `indisvalid = false` ⇒ **(A) confirmado**, y el fix es `DROP INDEX CONCURRENTLY` + recrear (**ya documentado en `113:24-26`**).
3. **Si es (B)**, el fix NO es en `sim-ctl`: hay que decidir si la retención debe respetar `ON DELETE CASCADE` sobre `simulations` (`004:6`) — **territorio de `scripts/`, fuera de mi alcance**.

**Y una discrepancia que dejo declarada, porque contradice el dato de entrada:** la migración `113:17` afirma que `simulations` es *"a populated live table (~640k rows)"*, mientras la medición reporta **0 filas históricas**. Con `ON DELETE CASCADE` (`004:6`) eso es **explicable** (la retención purgó `opportunities` y la cascada se llevó `simulations`), pero implica que **la inferencia "sin una sola escritura en toda la historia" es falsa**: hubo filas y fueron borradas en cascada. Eso **refuerza (B)**. Lo reporto en vez de armonizarlo.

---

## 9. ALCANCE Y P/N

**NO mueve P/N (0/115).** Sin firma, sin broadcast, sin deploy, sin capital. **No se tocó el sizing ni ningún límite de riesgo.** Único path: este documento.

*Cadena reconstruida sobre `main @ c89d21a3`; 8 eslabones citados `archivo:línea`; la mitad "nunca se llama" refutada por estructura; la mitad "falla" separada en dos mecanismos excluyentes que el canal denegado impide discriminar. Se declara el bloqueo en vez de elegir un ganador por plausibilidad.*
