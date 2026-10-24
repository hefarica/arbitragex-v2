ID:            REA-10-PER-LEG-LEDGER
Fuente:        hummingbot/hummingbot @9af100d6822da7d2d0291a906c730ef172284ee2
               hummingbot/strategy_v2/executors/ (executor_base.py, data_types.py, position_executor/,
               arbitrage_executor/, xemm_executor/) — ejecutores autocontenidos con su propio estado/PnL
Licencia:      Apache-2.0 (LICENSE del sha: "Apache License Version 2.0") — sin copia (ver Tipo)
Tipo:          INSPIRACION (invariante de diseño; NO hay fórmula numérica portable al destino)
Invariante:    cada pata/ejecutor mantiene su propio registro (entrada/salida por hop) y el resultado de la
               ruta se deriva de la cadena completa — nunca de un agregado que pierda la pata.
Matematica:    n/a. El artefacto verificable en el destino es el par (leg_amounts_in, leg_amounts_out) por
               candidato, con attach_leg_ledger all-or-nothing (longitudes iguales o no se adjunta).
Test origen:   n/a (hummingbot aporta el patrón de ejecutor autocontenido, no un número reejecutable contra
               nuestro motor). Se declara en vez de inventarse.
Test propio:   NINGUNO NUEVO — esta unidad no escribe código. Evidencia por medición ejecutada (abajo).
Gate:          NO aplica unidad nueva. Medición ejecutada en esta sesión sobre el destino (grep con rutas+línea):
               · size_optimizer.rs: 95 referencias a leg_amounts_* (producción del ledger por pata en los
                 kernels, incluido el re-derive al tamaño Kelly-capped de HOPS-LEDGER-05 / reledger_at :1398)
               · shared-rs/src/candidates.rs: 44 refs (tipo compartido con los campos por pata)
               · orchestrator.rs:1973-1996 (attach_leg_ledger all-or-nothing sobre el candidato elegido)
               · cartridge_boot.rs:4229/4275-4277/4467-4470 (mismo contrato en el path cartridge)
               · persistence.rs:282 (+9 refs): se persiste al emitir cuando el kernel produjo wei por pata
               · route_discovery/hop_cycle_bridge.rs: 6 refs — el productor VIVO del scanner adjunta el ledger
                 del par cruzado (CROSSED-MARKET / TWO-LEG-01)
               · opportunity_emitter.rs:880-894: PINNEA leg_amounts_in[0] como amount_in del registro
               · economics.rs:143/287-288: consume el ledger para el neto
               → el ledger por pata no solo existe: está cableado en producción, en el emitter y en la DB.
Control neg.:  n/a. La versión "sin ledger" es anterior a HOPS-LEDGER-05 (339 de 4.16M filas en 24h llevaban
               ledger en el momento de la medición que motivó el fix); hoy la cadena es intocable por contrato.
Evidencia:     greps ejecutados en esta iteración sobre backend/ con las líneas listadas; hummingbot clonado,
               sha y LICENSE leídos.
Riesgo:        n/a (cero cambios de código).
FOLLOWUP:      (a) SI se quisiera PnL USD POR PATA (hoy los montos son wei in/out por hop, sin valoración por
               pata) — requeriría precios por leg; declarado, no implementado. (b) El state machine de executor
               autocontenido NO se porta: el destino no ejecuta órdenes en vivo (paper/shadow).
Estado:        BLOCKED — razón: YA SATISFECHO por medición (premisa del backlog stale; no hay gap portable)
