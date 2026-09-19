# ArbitrageX v2 — Evolución desde Abril 2026 hasta el umbral del primer passed=true
> Emitido 2026-09-17 por el orquestador. Fuentes: git (1.645 commits, 2026-04-21 → hoy),
> ledger de memoria de sesiones (MEMORY.md, ~80 misiones), LEARNINGS.md del gang,
> queries PG/Redis vivas del día. Sin narrativa inventada: cada época cita su evidencia.

## Línea de tiempo por épocas

### Época I — FUNDACIÓN (abril-mayo, 600 commits): el esqueleto
- 2026-04-21 primer commit: "Sprint 1 Foundations + Sprint 2 Detection Real".
- Mayo = mes más fértil (554 commits): arquitectura C-S-E (collector Rust, strategy TS,
  edge, executor), Docker, first pipelines. La DApp nace como papel/shadow por diseño.
- **Estado del sueño: 0% — existe código, no existe ciclo.**

### Época II — CARNE Y PRIMEROS CICLOS (junio-julio, 558 commits): el embudo toma forma
- Cartuchos/estrategias proliferan; migración cartridge v3 (Rhai, boot-race P0 resuelto).
- Detección real (mempool, reserves), Postgres/Redis consolidados, panel deploy lock.
- 2026-07-28: handoffs ×4 de sesiones paralelas — el trabajo multi-agente se vuelve norma.
- **Evolución: detección viva, pero sim/validación aún sin forma. 0 sims aún (tabla nace 08-08).**

### Época III — EL AÑO DEL EMBUDO (agosto, 335 commits + nace la métrica que juzga todo)
- **2026-08-08: primera simulación de la historia** (tabla `simulations` nace).
- Agosto es un péndulo de avance y catástrofe gestionada:
  - ✅ MC-CRED/RPC cerrados, V-AT-1, G-SIM-1 topología viva, A.4 fork+anvil, FE-MASTER 83 secciones.
  - 🔥 Incendios: LOGFLOOD (falso deadlock por 183 líneas/s), FREEZE-01+02 (purge sin lock),
    WAL-burst DELETE → disco 100%, paper executor tradeando REJECTED (R-0001 — la lección
    de "nunca re-etiquetar"), viable=0 honesto.
  - 🧯 Doctrina HARDENING §37 (08-13): la carga de la prueba es del cambio.
- **A fin de agosto: el embudo existe completo, pero 0 passed en ~1M de sims.**

### Época IV — CAZA DE LA PRIMERA PASSED (septiembre, 152 commits y contando)
Cadena de cuellos descubiertos y decapitados, en orden:
1. **HG certificación (09-06): 0/10 gates, 992K sims, 0 passed.** Vault SEALED. Flip rechazado.
2. **XEN exclusion (09-16):** XEN era el 99.86% de TODO el flujo — el flood enmascaraba
   que el embudo real producía ~0. Al matarlo: queda la verdad desnuda.
3. **routes_found=0 (09-16):** DFS quemaba 100K visitas en tokens hoja antes de tocar WETH.
   (Hoy: `work_limited:false`, lat_candidates fluyendo — resuelto.)
4. **v3_quote_unavailable:** fix de transporte (#575/#576 quote cache, deployado);
   el residuo de "cobertura" resultó ser **2 pools muertas** (liquidity=0 on-chain,
   desactivadas hoy con evidencia).
5. **HOY (09-17):** deploy `06eba18e` = SIM-FUND-01 (fondeo del probe: mataba el 55% de
   las sims con STF) + PANCAKE-ROUTER-01 + SEL-GATE-01. Y en CI: **#580 SIM-FUND-01b** —
   el bug de 12 bytes de padeo que dejó el fondeo inerte (706/706 slot_unresolved),
   cazado por contraste con vector externo y validado por Hermes.

## ¿Evolución o involución? — el veredicto honesto

**Ambas, y la involución fue el precio de la evolución real.**
- **Involución aparente:** de "554 commits/mes" a 152/mes; de promesas de dashboard a
  "0 passed en 2M sims"; agosto quemó días en incendios auto-infligidos (LOGFLOOD,
  FREEZE, WAL). La DApp pasó de maqueta ambiciosa a sistema honesto que reporta su propia miseria.
- **Evolución real (la que importa):** cada mes eliminó una CLASE entera de ceguera:
  abril-mayo dio cuerpo, junio-julio dio ojos (detección), agosto dio juicio (embudo
  medible + doctrina anti-regresión), septiembre está dando el primer CICLO. Un sistema
  que hoy dice "0 de 2M" con precisión vale más que uno que en mayo decía "todo verde"
  sin poder probar un solo ciclo.

## A HOY — a las puertas del primer passed=true (evidencia del día)

| Capa | Estado verificado hoy |
|---|---|
| Detección | ✅ viva (28.3M oportunidades; última hace minutos) |
| Grafo/rutas | ✅ lat_candidates 3-hop, reprice ~1ms, work_limited=false |
| Catálogo | ✅ 2 pools muertas fuera; PANCAKE router integrado (deploy) |
| Simulación | ✅ 7.755 sims/24h fluyendo al simulador |
| Funding del probe | 🔄 fix del bug de padeo en CI (#580) — EL último eslabón conocido |
| Gates vivos | `passed`: **0 de 2.031.513** (40 días, 16h) |
| Flota | 24/24 healthy en `06eba18e` |

**Lo que falta para passed=true:** CI #580 verde → merge → rebuild sim-ctl → up -d.
Minutos-a-horas, no días. Y si aún no pasa: el siguiente sospechoso ya está fichado
(`zero_amount_in` residual H2 + clase D-SIM-01) — el embudo ya no tiene zonas oscuras.

## La línea de fondo

Abril: una idea. Hoy: una máquina que detecta, cotiza, simula y se financia a sí misma
en fork, con cada mentira posible ya ejecutada y eliminada. La primera passed=true no
será suerte — será la 2.031.514ª simulación, la primera con TODOS los eslabones vivos.
