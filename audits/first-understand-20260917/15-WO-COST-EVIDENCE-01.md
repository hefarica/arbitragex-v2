# WO-COST-EVIDENCE-01 — Cascada §39 como artefacto obligatorio + gate mecánico (orden operador 2026-09-18)
> Origen: diseño del operador (invariantes + artefacto + candados mecánicos), mapeado honesto al stack ARBX.
> Memoria vigente: "ninguna acción sobre settings.json" — los hooks del diseño original NO aplican (framework ajeno).

## Mapeo de las 3 invariantes al stack real
- **INVARIANTE_COSTO** → atomicidad PG: el INSERT de la opportunity ya ES atómico. Extensión: filas SIZED persisten
  `cost_breakdown` JSONB (componentes del kernel: gas, ops, TLS fee, gross, net) EN EL MISMO INSERT (all-or-nothing gratis).
  Σcomponentes ≡ total verificado por query/gate.
- **INVARIANTE_SIM_SIEMPRE** → forwardSimulate YA corre en toda fila (read-time, HARDENING 2026-08-22). Gap real = inputs
  (gross nulo en rechazos pre-economía / token sin precio). Frescura: `input_hash = sha256(config_snapshot ∥ row inputs)` —
  cache reutilizable ⇔ hash idéntico; si no ⇒ recomputar. NUNCA servir sim con config vieja.
- **INVARIANTE_EVIDENCIA** → gate mecánico: `automation/gate-cost-check.sh` (o test CI) — toda fila sized reciente DEBE
  llevar breakdown con Σcomponentes == total y hash de contenido verificado; accepted ⇒ breakdown EXISTE. exit≠0 ⇒ CI rojo,
  nada llega al VPS. Evidence-over-narrative: narrativa sin registro = no cuenta.

## Artefacto (esquema fijo, análogo al propuesto)
`opportunities.cost_breakdown` JSONB: {p_gas, fee_mult, components[{etapa,gas,costo}], gas_total, sim:{fresh,input_hash}, hash}
- hash = sha256 del contenido canónico del bloque; el gate lo RECOMPUTA (jamás confía en el campo).
- R8 intacto: rechazadas PRE-economía (sin gross) NO fabrican desglose — quedan 'not computed:<razón>' hasta que
  probe_economics (WO-CARDS PR-C) les dé gross real.

## Candados mecánicos (equivalentes en nuestro stack)
1. persist atómico = INSERT único PG (ya existe; extender columnas).
2. require-cost-evidence = job CI "cost-evidence gate" + checker query en L4 post-deploy.
3. MAX_RETRY = kill-switch del emisor ya existe (no aplica retry-loop aquí; el gate falla el pipeline, no reintenta).

## No mapea (honesto)
- hooks settings.json (framework ajeno + directiva operator "no tocar settings.json").
- grafo episódico / DEAD_END / backtrack: no existen; SSOT = PG + Redis + BOARD.

## Pendiente: validación Sancho (doctrina §12.4) — despachada.
