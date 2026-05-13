---
name: sop_executive_summary
description: Cuando el usuario pregunta por qué ArbitrageX es competitivo, qué lo diferencia de competidores con ethers-rs, o cuál es la tesis central del proyecto. Activa esta skill cuando aparezcan triggers "ventaja competitiva", "por qué Alloy", "por qué 95% pierde", "asimetría de información", "tesis ArbitrageX". Sintetiza el resumen ejecutivo del SOP_ArbitrageX_2026.pdf §1 con los 4 pilares operativos.
type: arbx_strategy_reference
source_section: SOP_ArbitrageX_2026.pdf §1
---

# Resumen Ejecutivo ArbitrageX

## Tesis central
**El 95% de los buscadores MEV pierde dinero sistemáticamente.** Las tres causas:
1. Latencia inadecuada (REST APIs, ethers-rs con copies innecesarias).
2. Falta de simulación precisa pre-broadcast.
3. Arquitectura que no escala horizontalmente.

ArbitrageX usa **Alloy v0.9** (Paradigm) en lugar de ethers-rs porque ofrece decodificación zero-copy + integración nativa con `revm`.

## 4 pilares competitivos (del SOP §1)
1. **Ejecución atómica** — bundles + flash loans → cero capital inicial.
2. **Escaneo sub-milisegundo** — WebSocket + Alloy → mempool + on-chain en tiempo real.
3. **Simulación determinista** — `revm 19.0` + `alloy-provider` → estado real pre-broadcast.
4. **Asimetría de información** — multi-DEX + multi-CEX + multi-pool seguimiento simultáneo.

## Patrón arquitectónico C-S-E (Compose-Simulate-Execute)
- **Compose**: grafo de rutas de arbitraje desde pools de liquidez.
- **Simulate**: cada candidata simulada localmente con revm + estado on-chain real.
- **Execute**: bundle atómico vía Flashbots / MEV-Boost relay si profit_neto > umbral.

## Cuándo invocar este conocimiento
- Antes de aprobar un PR que añade dependencia de `ethers-rs` (→ rechazar, usar Alloy).
- Antes de aprobar simulación que NO use revm (→ rechazar, falta determinismo).
- Antes de broadcast a mempool público (→ rechazar, debe ir por Flashbots/MEV-Boost).
- Cuando se discuta tesis del proyecto con stakeholders.

## Invariantes inmutables
- NUNCA broadcast a mempool público para arbs propios.
- NUNCA simular sin estado on-chain real (revm + alloy-provider).
- NUNCA usar ethers-rs en hot-path nuevo (deprecado oct/2023).

## Cross-references
- Stack técnico: ver `sop_csa_architecture` (cap 2 SOP).
- Matriz de estrategias: ver `sop_strategy_matrix` (cap 3 SOP).
- Código Alloy de referencia: ver `sop_dex_triangular` (cap 4 SOP §4.3).
