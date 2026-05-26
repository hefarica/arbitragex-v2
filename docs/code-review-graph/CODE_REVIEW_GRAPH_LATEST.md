# CODE REVIEW GRAPH — LATEST

## 1. Resumen ejecutivo
- Fecha: 2026-05-21
- Rama: feat/omega-scaffold-v2 (working dir) · target `main` @ `d9bea59`
- PR en foco: #87 (sed-core feature gates) — baseline PRE-CHANGE
- Iteración: **1 (baseline real, subconjunto nativo sin Docker)**
- Estado global: **WARN** (hallazgos reales medios + cobertura parcial)
- Decisión: **GO con deuda documentada** (ningún secreto, ningún crítico-RCE; advisories medios/transitivos a trackear)
- Score anterior: n/a · **Score actual: 70/100** (provisional — 3 capas BLOCKED) · Variación: n/a
- Riesgos nuevos: 8 RUSTSEC advisories + 7 crates unmaintained/unsound (baseline)
- Riesgos corregidos: 0 (primera medición real)
- Riesgos persistentes: n/a (baseline)

## 2. Matriz por herramienta (REAL, sin inventar)
| Herramienta | Estado | Nuevos | Corr. | Pers. | Bloq. | Evidencia |
|---|---|---:|---:|---:|---:|---|
| CodeQL | PASS (CI) | 0 | 0 | 0 | 0 | checks `CodeQL`/`analyze (rust\|ts)` verdes en main; CLI local AUSENTE |
| Joern | **BLOCKED** | – | – | – | – | no instalado; JVM pesado en Windows; sin Docker |
| Semgrep | **PASS** | 0 | 0 | 0 | 0 | `p/secrets`, 41 reglas / 279 archivos → **0 findings**. `docs/code-review-graph/sarif/semgrep-secrets.json` |
| dependency-cruiser | WARN | – | – | – | – | instalado; falta `.dependency-cruiser.cjs`+tsconfig (cruzó 0 módulos sin config) — pendiente config |
| Slither | **BLOCKED** | – | – | – | – | requiere `solc` + soporte Windows pobre; sin Docker. Cobertura Solidity vía CI `forge test` |
| Foundry | PASS (CI) | – | – | – | – | `forge` AUSENTE local; check `lint-and-test-contracts` verde en CI |
| RustSec (cargo-audit) | **FAIL** | **8 vuln + 7 warn** | 0 | – | 0* | `cargo audit` backend/ (761 deps). Detalle §4 |
| Syft | **BLOCKED** | – | – | – | – | no instalado; SBOM-de-imagen imposible sin Docker. Pendiente binario Windows para SBOM filesystem |

\* Ninguna advisory marcada crítica/high-RCE explotable → no bloquea merge (deuda de supply-chain a trackear).

## 3. Cambios analizados
- Archivos modificados (esta iteración): solo `docs/` (FASE-0 OMEGA + este gate). **Cero cambios de código de app** aún.
- Capas: documentación. Workflows: ninguno. Dependencias: ninguna nueva. Contratos: ninguno. Crates: ninguno modificado.

## 4. Hallazgos nuevos — RustSec (backend/Cargo.lock, 761 deps)
| ID | Crate | Severidad | Título | Causa raíz | Acción |
|---|---|---|---|---|---|
| RUSTSEC-2023-0071 | rsa | Medium 5.9 | Marvin Attack (timing sidechannel key recovery) | transitive vía firma/cripto | trackear; ¿uso en path sensible? |
| RUSTSEC-2024-0363 | sqlx | Medium | Binary Protocol Misinterpretation (casts truncados) | sqlx versión afectada | upgrade sqlx |
| RUSTSEC-2024-0437 | protobuf | Medium | Crash por recursión no controlada (DoS) | transitive | upgrade protobuf |
| RUSTSEC-2025-0009 | ring | Low/Med | AES panic con overflow-checking | transitive (ring<0.17) | upgrade ring≥0.17 |
| RUSTSEC-2026-0098 | rustls-webpki | Medium | name constraints URI aceptados | transitive | upgrade rustls-webpki |
| RUSTSEC-2026-0099 | rustls-webpki | Medium | name constraints wildcard | transitive | upgrade rustls-webpki |
| RUSTSEC-2026-0104 | rustls-webpki | Medium | panic en parsing de CRL (×2 paths) | transitive | upgrade rustls-webpki |
| (raíz común) | **ethers 2.0.14** | — | librería **deprecada** que arrastra varias de las anteriores | dep directa | migrar a `alloy` (parcialmente en curso: ya hay `alloy-*`) |

**Warnings (7, unmaintained/unsound):** derivative (RUSTSEC-2024-0388), fxhash (2025-0057), instant (2024-0384), paste (2024-0436), ring<0.17 (2025-0010), rustls-pemfile (2025-0134), lru unsound (2026-0002).

**Lectura:** la mayoría son **medios, DoS/panic o sidechannel, transitivos** — varios provienen de `ethers 2.0.14` (deprecada). No hay RCE crítico ni secretos. El fix estructural es **completar la migración ethers→alloy** y bumpear `sqlx`/`rustls-webpki`/`protobuf`/`ring`.

## 5–6. Corregidos / Persistentes
Baseline: sin correcciones aún; estos 8+7 quedan como **persistentes a trackear** en próximas iteraciones.

## 7. Grafo de arquitectura
dependency-cruiser pendiente de config (`.dependency-cruiser.cjs`) — sin datos de ciclos/boundaries esta iteración (WARN).

## 8. Seguridad y supply chain
- Secrets: **0** (semgrep p/secrets, 279 archivos). ✅
- SAST: semgrep base limpio en dirs escaneados; falta ampliar a `--config auto` (job más largo).
- Dependencias: 8 RUSTSEC + 7 unmaintained (Rust). npm/JS audit no corrido esta iteración.
- SBOM: pendiente (Syft no instalado).
- Advisories: §4.

## 9. Contratos / Rust / MEV
- Solidity/Foundry/Slither: vía CI (`forge test` verde); Slither local BLOCKED.
- Rust: 8 advisories (§4). Live-trading gate y kill-switch: gobernados por skills `arbx-*` (no tocados esta iteración).

## 10. Decisión
**GO con deuda documentada.** No hay bloqueante (0 secretos, 0 crítico-RCE). Los 8 advisories son medios/transitivos → backlog (migración alloy + bumps). Cobertura **parcial** (Slither/Joern/Syft/CodeQL-local BLOCKED en Windows sin Docker) → score provisional.

## 11. Próxima acción
- Config `.dependency-cruiser.cjs` para activar grafo de arquitectura.
- (Opcional) instalar Syft (binario Windows) para SBOM filesystem.
- Abrir tarea de migración `ethers→alloy` + bumps `sqlx/rustls-webpki/protobuf/ring`.
- Continuar #87 bajo este gate (re-correr cargo-audit/semgrep en la superficie tocada).
