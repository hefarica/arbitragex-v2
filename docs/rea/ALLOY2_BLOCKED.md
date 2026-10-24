# ALLOY2 — RUSTSEC-2026-0387 **no se cierra** migrando alloy 1→2 (medido, con evidencia)

Fecha: **2026-10-24** · Autor: verificador (t24) · Repo: `hefarica/arbitragex-v2` · Base: `origin/main` @ `a3d34263`.
Unidad: ALLOY2 (fix REAL de RUSTSEC-2026-0387 vía migración mayor alloy 1.x → 2.x).

> **Veredicto de esta unidad: BLOQUEADA POR PREMISA FALSA.** La cadena de fix que el encargo describe
> (`alloy 2.x → alloy-signer 2.5.0 → k256 0.14.0 → elliptic-curve ^0.14.1 → crypto-bigint 0.7.6`) **no existe**:
> medido hoy, **alloy 2.5.0 resuelve k256 0.13.4 / elliptic-curve 0.13.8 / crypto-bigint 0.5.5**. Además
> `alloy-primitives` y `alloy-sol-types` **no tienen línea 2.x** (última: 1.7.3), y el otro consumidor de k256 0.13
> es **ethers-rs 2.0.14**, que es el firmante REAL del sistema. Conclusión: **hacer la migración de alloy no remueve
> la vulnerabilidad** y quemaría una migración grande + riesgo en el camino de firma sin mover el gate.

---

## 0. Estado operativo inmediato (lo que importa al capitán HOY)

`main` (`a3d34263`) tiene el gate de seguridad **ROJO**: workflow *Security Scans* → job **`cargo audit (Rust advisories)` = failure**
(run `40063839526`, job `120782809345`). Extracto literal del log:

```
    Scanning Cargo.lock for vulnerabilities (815 crate dependencies)
Crate:     crypto-bigint
ID:        RUSTSEC-2026-0387
URL:       https://github.com/RustCrypto/crypto-bigint/pull/1365
crypto-bigint 0.5.5
error: 1 vulnerability found!
```

La lista vigente de ignores de ese job **no incluye 0387** (últimas: `…0381 …0382 …0383`; 26 en total) y el allowlist
declara `RUSTSEC_IGNORES allowlist valid until 2030-12-31 (today 2026-10-24)`.
⇒ El **puente firmado (ignore justificado)** es hoy la ÚNICA vía para poner el gate en verde; el fix real no está disponible
en el ecosistema (ver §2-§4). Hay una rama en vuelo `docs/rustsec-ignore-evidence` (`a6ed2e0f`) que apunta a eso.

---

## 1. La advisory (fuente autoritativa, citada)

`https://raw.githubusercontent.com/rustsec/advisory-db/main/crates/crypto-bigint/RUSTSEC-2026-0387.md` (fetch 2026-10-24, HTTP 200):

```toml
[advisory]
id = "RUSTSEC-2026-0387"
package = "crypto-bigint"
date = "2026-10-23"
url = "https://github.com/RustCrypto/crypto-bigint/pull/1365"
categories = ["crypto-failure"]
[versions]
patched = [">= 0.7.6"]
```

> *Multiple timing variability issues in operations intended to be constant-time* — subtraction, zero counting, division,
> Montgomery multiplication, `UintRef` equality e `invert`. **Impacto**: fuga por canal lateral de **enteros secretos**
> (recuperación potencial de secretos criptográficos, dependiendo de algoritmo/compilador/CPU/atacante).
> **Resolución**: v0.7.6.

Es una advisory de **hoy mismo** (2026-10-23): no existía cuando se planificó la Fase 1.

---

## 2. Versiones reales en crates.io (medido con `cargo info`, 2026-10-24)

| crate | última versión | ¿existe 2.x? |
|---|---|---|
| `alloy` | **2.5.0** | **SÍ** (2.x existe) |
| `alloy-provider` / `alloy-network` / `alloy-signer` / `alloy-signer-local` | **2.5.0** | **SÍ** |
| **`alloy-primitives`** | **1.7.3** | **NO** — el encargo pedía "alloy-primitives (2)": no existe |
| **`alloy-sol-types`** | **1.7.3** | **NO** — el encargo pedía "alloy-sol-types (2)": no existe |
| `crypto-bigint` | **0.7.6** | (0.7.x = parche) |
| `k256` | **0.14.0** | — |
| `elliptic-curve` | **0.14.1** | — |

**Corrección #1 al encargo:** el bump coordinado "alloy-primitives (2) / alloy-sol-types (2)" es **imposible**: esas dos
crates siguen en 1.x. El bump real sería **sólo** la familia `alloy`/`alloy-provider`/`alloy-network`/`alloy-signer*`.

---

## 3. Lo que REALMENTE resuelve alloy 2.5.0 (probe reproducible)

Proyecto limpio con `alloy = "2"` (features `providers, transport-http, rpc-types-eth, network, sol-types, rlp`) +
`alloy-primitives = "1"` + `alloy-sol-types = "1"`; `cargo generate-lockfile` + `cargo tree`:

```
--- lock resuelto ---
alloy              version = "2.5.0"
alloy-signer       version = "2.5.0"
alloy-primitives   version = "1.7.3"
alloy-sol-types    version = "1.7.3"
k256               version = "0.13.4"      <-- NO 0.14
elliptic-curve     version = "0.13.8"      <-- NO 0.14
crypto-bigint      version = "0.5.5"       <-- EL VULNERABLE SIGUE

--- quién lo requiere (cargo tree -i crypto-bigint) ---
crypto-bigint v0.5.5
└── elliptic-curve v0.13.8
    ├── alloy-signer v2.5.0
    │   ├── alloy v2.5.0
    │   ├── alloy-network v2.5.0
    │   ├── alloy-provider v2.5.0
    │   └── alloy-contract v2.5.0 …
```

**Corrección #2 (decisiva):** `alloy-signer 2.5.0` **depende de k256 ^0.13** (su `Cargo.toml` lista `k256` y `elliptic-curve`,
y el lock resuelve 0.13.4). La cadena "signer 2.5 → k256 0.14" del encargo **es falsa**. Además, en alloy 2.5
**`alloy-primitives` 1.7.3 también arrastra k256 0.13.4**.

---

## 4. Lo que pasa en el workspace REAL (no en el probe)

`backend/Cargo.lock` (origin/main `a3d34263`):

```
crypto-bigint    0.5.5      <- única copia, la vulnerable
k256             0.13.4
alloy            1.8.3 ;  alloy-signer 1.8.3 ;  alloy-primitives 1.6.1
ethers           2.0.14 ;  ethers-core 2.0.14 ;  ethers-signers 2.0.14
```

`cargo tree -i k256 --locked` (en `backend/`, WSL, sin compilar) — dependientes reales de k256 0.13.4:

```
k256 v0.13.4
├── alloy-consensus v1.8.3        (→ alloy, → todos los crates del workspace)
├── alloy-signer v1.8.3
├── alloy-signer-local v1.8.3
├── alloy-primitives v1.6.1
└── ethers-signers v2.0.14        (→ ethers v2.0.14, → ethers-middleware v2.0.14)
```

**Corrección #3 (bloqueante):** aunque se eliminara toda la familia `alloy`, **ethers-signers 2.0.14 sigue exigiendo
k256 0.13.4 ⇒ crypto-bigint 0.5.5 permanece**. Y ethers-rs es el **firmante real** del sistema
(`relays-client/src/signer.rs`, `relay_flashbots.rs`, según la evidencia de no-reachability de t23).
⇒ **No hay bump de versiones disponible hoy que cierre la advisory.**

---

## 5. Opciones reales (con esfuerzo y riesgo)

| # | Opción | ¿Cierra 0387? | Esfuerzo / riesgo |
|---|---|---|---|
| **O1** | **Puente firmado hoy**: `--ignore RUSTSEC-2026-0387` con justificación + fecha de re-review (rama en vuelo `docs/rustsec-ignore-evidence`) | No (mitiga el gate) | Bajo. **Es la única vía disponible hoy**; el allowlist global vence 2030-12-31 → conviene una re-review específica antes |
| **O2** | **Esperar upstream**: alloy y ethers migren a k256 0.14 / elliptic-curve 0.14 | **Sí**, cuando ocurra | Nulo hoy (no está). Seguimiento: issues/PRs de alloy-rs/alloy y gakonst/ethers-rs |
| **O3** | **Eliminar TODO el stack criptográfico**: quitar el uso de alloy-signer/consensus Y reemplazar ethers-signers por firma directa sobre **k256 0.14** (+ encoding con alloy-primitives) | **Sí** (no quedaría dependiente de k256 0.13) | **Alto**: toca el camino de FIRMA (adyacente a ruta protegida), requiere vectores de firma/recuperación, y es una unidad propia (no ALLOY2) |
| **O4** | `[patch.crates-io]` con un fork de crypto-bigint 0.5 backport | No recomendable | Rechazada: k256 0.13 no acepta la API 0.7, y meter un fork en la cadena de firma es riesgo de supply-chain |

**Recomendación:** O1 ahora (desbloquea el gate sin relajar la doctrina: ignore firmado y con re-review), y **O3 como unidad
propia** si el operador quiere el cierre real antes de la expiración del ignore — con O2 como salida natural si upstream se mueve
primero. **ALLOY2 tal como está escrito NO debe ejecutarse**: es una migración mayor que no mueve la advisory.

---

## 6. Lo que SÍ queda verificado en esta unidad (evidencia utilizable)

1. **La premisa del encargo es falsa en tres puntos** (§2, §3, §4), cada uno con su medición reproducible.
2. **`main` está rojo por esta advisory**, con el log literal (§0) — el capitán necesita el puente para desbloquear el deploy.
3. **Superficie de alloy en el workspace, medida** (por si algún día se retoma O3 o el bump parcial):
   **42 ficheros `.rs`** usan `alloy`; APIs: `primitives::Address` (29), `providers::Provider` (16),
   `rpc::types::TransactionRequest` (11), `sol!`/`SolCall` (9), `ProviderBuilder` (3), `Uint`/`B256`/`Bytes`/`Log`/`Ethereum`/`BlockId`.
   Crates con alloy en su Cargo.toml: `math-engine`, `recon`, `relays-client`, `searcher-rs`, `shared-rs`, `simulator-v2`, `token-enricher`.
4. **Reproductor exacto** de todo lo anterior (sin tocar el repo):

```bash
# versiones
cargo info alloy alloy-primitives alloy-sol-types alloy-signer k256 elliptic-curve crypto-bigint
# probe (alloy 2.5.0) — demuestra que crypto-bigint 0.5.5 SIGUE
mkdir -p /tmp/alloy2probe/src && cd /tmp/alloy2probe
#   Cargo.toml: alloy="2" (providers,transport-http,rpc-types-eth,network,sol-types,rlp)
#               alloy-primitives="1"  alloy-sol-types="1"
cargo generate-lockfile && cargo tree -i crypto-bigint && cargo tree -i k256
# workspace real (sólo resuelve, no compila)
cd backend && cargo tree -i k256 --locked
```

---

## 7. Restricciones respetadas

No se tocó ningún archivo del repo en producción de código: esta unidad **no modificó** `Cargo.toml`/`Cargo.lock` (no hay bump
que hacer que cierre la advisory), ni rutas protegidas (`configs/app.toml`, `live_exec_policy.rs`, broadcast path de
`bundle_builder.rs`), ni `paper_mode`, ni la DB. No se relajó ningún gate: **no se añadió ningún ignore** — esa decisión es del
operador (puente firmado).

## 8. Fail-honest

- **No ejecuté la migración alloy 1→2** porque está medido que **no cierra** RUSTSEC-2026-0387 y sí introduce riesgo en un
  workspace de 42 ficheros con API de provider/TransactionRequest que cambia entre mayores; hacerla «para cumplir» sería trabajo
  teatral con el gate igual de rojo.
- **No corrí `cargo audit` localmente** (no hay `cargo-audit` en el entorno de trabajo); la evidencia del fallo es el **log del
  job de `main`** (§0), que es la fuente autoritativa de CI.
- **No verifiqué** si `alloy 2.5.0` rompe compilación en nuestro código: no lo medí porque el resultado (compilación) no cambia la
  conclusión sobre la advisory. Si el operador quiere el bump por otras razones (features, bugs), es una unidad distinta y habría
  que hacerla con build completo.
- La severidad/exploitability **real** de la advisory en nuestro sistema no la declaro: la advisory es de canal lateral y su
  explotabilidad depende de compilador/CPU/atacante; lo que sí está medido es **dónde vive el crate** y **que no hay versión
  parcheada al alcance hoy**.
