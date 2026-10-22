ID:            REA-09-DETERMINISTIC
Fuente:        nautechsystems/nautilus_trader @a51ae015c150ef68f62d9295e3efa9534f492a4a
               crates/common/src/clock/mod.rs:50 (trait Clock — reloj explícito inyectado)
               docs/concepts/backtesting/index.md:3 ("same core components as live trading")
               docs/concepts/backtesting/execution-flow.md:219 ("ts_init is pinned in backtest data and
               monotonic in live/sandbox")
Licencia:      LGPL-3.0 (LICENSE del sha: "GNU LESSER GENERAL PUBLIC LICENSE Version 3") — sin copia de código
Tipo:          INSPIRACION (invariante arquitectónico; NO existe fórmula ni vector numérico portable)
Invariante:    el MISMO código de componentes corre en backtest y en vivo; el entorno difiere SOLO en el Clock
               (reloj explícito inyectado), de modo que un replay con el mismo input produce las mismas decisiones
               y el único campo dependiente del entorno es el timestamp de evidencia.
Matematica:    n/a — el artefacto verificable es (1) módulo de simulación COMPARTIDO por los dos caminos
               (predicción y admisión) y (2) ausencia de fuentes de tiempo no inyectadas en las DECISIONES.
Test origen:   n/a (nautilus no aporta un número que se pueda reejecutar contra nuestro motor; sus tests son
               de su propio engine). Se declara en vez de inventarse.
Test propio:   NINGUNO NUEVO — esta unidad no escribe código. Evidencia por medición ejecutada (abajo).
Gate:          NO aplica unidad nueva. Medición ejecutada en esta sesión sobre el destino:
               · sim_core es el motor ÚNICO: searcher-rs/src/candidate_simulation.rs:175/189/234/424
                 (sim_encoder + verified sim) y relays-client/src/execution_admission.rs:90-116
                 (sim_core::verified_simulation::execute) — mismo engine en predicción y admisión.
               · replay real: sim-core/tests/variance_benchmark.rs:27 re-ejecuta el path de PRODUCCIÓN
                 (execute_multistep_revm) dos veces sobre oportunidades REALES exportadas (nunca sintéticas),
                 B y B+1; #[ignore] por defecto (requiere RPC), fail-honest si falta env.
               · disciplina de determinismo ya presente: op_22 Monte Carlo sembrado desde block_number
                 (operators/op_22_monte_carlo.rs:111 "NO Date::now"), op_32 :967 "(5) Determinismo: mismo
                 estado ⇒ salida bit-identica", real_ops_tests.rs:76, omega-search/graph.rs:596/622,
                 bayesian_allocator.rs:475-479 ("same (cap, yield) on a fresh ...").
Control neg.:  La ÚNICA fuente de reloj en el path de sim es verified_simulation.rs:183
               `SystemTime::now()` → se escribe en `simulated_at_ms` de SimulationEvidence DESPUÉS de todas las
               decisiones (passed/profit/gas en L130-181): no alimenta ninguna decisión. sim_encoder.rs:233
               documenta explícitamente "the encoder does NOT call SystemTime::now()". Si un día ese sello
               entrara en una decisión, el invariante se rompe — hoy no entra.
Evidencia:     greps ejecutados en esta iteración sobre backend/ (rutas+línea listadas); nautilus clonado y
               sha fijado; LICENSE leída.
Riesgo:        n/a (cero cambios de código). El único riesgo es de lectura: confundir "hay un now() en el
               archivo" con "hay un now() en la decisión" — medido y documentado.
FOLLOWUP:      (a) clock explícito estilo nautilus SOLO si el sello de evidencia contaminara decisiones
               (hoy no: sin valor operativo); (b) el test "mismo input ⇒ misma decisión" con REVM real ya
               existe como variance_benchmark (ignored, requiere RPC) — no se duplica.
Estado:        BLOCKED — razón: YA SATISFECHO por medición (no hay gap portable; premisa del backlog stale)
