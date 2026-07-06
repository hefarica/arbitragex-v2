# CODE REVIEW GRAPH — HISTORY

## Iteración 1 — 2026-05-21 (baseline, subconjunto nativo sin Docker)
- Estado: WARN · Decisión: GO con deuda documentada · Score: 70/100 (provisional)
- Ejecutadas: cargo-audit (RustSec), semgrep p/secrets. CI: CodeQL, Foundry. BLOCKED (Windows/sin Docker): Slither, Joern, Syft, dependency-cruiser(sin config).
- Hallazgos: 8 RUSTSEC vuln (medios/transitivos, raíz `ethers 2.0.14` deprecada) + 7 unmaintained/unsound; **0 secretos**.
- Bloqueantes: 0. Cambios de código de app: 0 (solo docs).
- Próximo: config dependency-cruiser; (opc) Syft binario; tarea ethers→alloy + bumps; continuar #87 bajo gate.
