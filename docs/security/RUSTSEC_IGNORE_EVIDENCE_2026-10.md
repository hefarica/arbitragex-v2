# RUSTSEC-2026-0387 / RUSTSEC-2024-0449 — paquete de evidencia para el ignore firmado

**Estado: PROPUESTA. `RUSTSEC_IGNORES` NO fue modificado.** Este documento reúne la evidencia que el operador
necesita para firmar (o rechazar) un puente temporal; la entry sólo se agrega a
`.github/workflows/security.yml` después de la firma.

Fecha: 2026-10-24 · Autor: integrador-2 (t23 CARGO-AUDIT) · Main de referencia: `a3d34263`
Unidad de remediación real: **t24 ALLOY2** (creada por el capitán).

---

## 1. Qué rompió el gate y cuándo

`security.yml` → job `cargo audit (Rust advisories)` pasó de **success** en main `3915f312`
(2026-10-23T23:23Z, job 120706588427) a **failure** en el PR de t21 (2026-10-24T04:03Z), sin cambios
Rust en ese diff: la base **RustSec** sumó advisories nuevas. El PR #1041 (t23) ya retiró una de ellas
(tokio) y el gate quedó con **exactamente 1 vulnerabilidad**:

```
$ cargo audit  # con los --ignore vigentes del workflow
error: 1 vulnerability found!
Crate:     crypto-bigint
ID:        RUSTSEC-2026-0387

warning: 2 allowed warnings found   # antes 3: tokio RUSTSEC-2026-0386 ya NO aparece
Crate:     unsafe-libyaml
Warning:   unmaintained
ID:        RUSTSEC-2024-0449
```

---

## 2. Textos literales de las advisories (RustSec advisory-db, `main`)

### RUSTSEC-2026-0387 — crypto-bigint

```toml
[advisory]
id = "RUSTSEC-2026-0387"
package = "crypto-bigint"
date = "2026-10-23"
url = "https://github.com/RustCrypto/crypto-bigint/pull/1365"
categories = ["crypto-failure"]
license = "CC-BY-4.0"

[versions]
patched = [">= 0.7.6"]
```

> Multiple timing variability issues in operations intended to be constant-time. Several operations
> intended to be constant-time could execute secret-dependent branches, depending on the compiler and
> target CPU. These include subtraction, zero counting, division, and Montgomery multiplication.
> Additionally, `UintRef` equality could terminate at the first unequal limb, and
> `BoxedMontyForm::invert` branched on whether an inverse exists.
> […] Practical exploitability depends on the downstream algorithm, compiler, target CPU, and an
> attacker's ability to observe the side channel. **Upgrade to v0.7.6.**

### RUSTSEC-2024-0449 — unsafe-libyaml

```toml
[advisory]
id = "RUSTSEC-2024-0449"
package = "unsafe-libyaml"
date = "2024-03-25"
informational = "unmaintained"
url = "https://github.com/dtolnay/unsafe-libyaml"

[versions]
patched = []
unaffected = []
```

> unsafe-libyaml is unmaintained (repositorio archivado 2024-03-25, última release 0.2.11). Es una
> traducción automática a Rust unsafe del C de libyaml; los bugs no se arreglan upstream. Lo usa
> `serde_yaml` (también deprecado). Alternativas: `saphyr` / `serde-saphyr`, `noyalib`.

---

## 3. Versiones resueltas y por qué no hay fix en semver

`backend/Cargo.lock` (main `a3d34263`):

| crate | versión | patched exigido | ¿fix en semver? |
|---|---|---|---|
| crypto-bigint | **0.5.5** | `>= 0.7.6` | **NO** |
| unsafe-libyaml | **0.2.11** | `[]` (sin parche) | **NO** (unmaintained) |

Intento literal de forzar el parche:

```
$ cargo update -p crypto-bigint --precise 0.7.6
error: failed to select a version for the requirement `crypto-bigint = "^0.5"`
candidate versions found which didn't match: 0.7.6
location searched: crates.io index
required by package `elliptic-curve v0.13.8`
    ... which satisfies dependency `elliptic-curve = "^0.13"` (locked to 0.13.8) of package `alloy-signer v1.8.3`
    ... which satisfies dependency `alloy-signer = "^1.8.3"` (locked to 1.8.3) of package `alloy v1.8.3`
    ... which satisfies dependency `alloy = "^1"` (locked to 1.8.3) of package `recon v0.1.0`
```

`alloy 1.8.3` es la **última 1.x**; `alloy-network 1.8.3` depende de `alloy-signer` de forma **no
opcional** (no se puede quitar por features). El parche exige `k256 0.14 → elliptic-curve ^0.14.1`,
que sólo viene con **alloy 2.x**… **y con una excepción crítica** (ver §4.2).

`unsafe-libyaml` entra por `serde_yaml 0.9.34+deprecated ← searcher-rs`; no tiene parche upstream.

---

## 4. Reachability real (corregida — esto cambia el alcance del plan)

### 4.1 alloy-signer (NO ejecutado por código propio)

```
crypto-bigint 0.5.5 ← elliptic-curve 0.13.8 ← alloy-signer 1.8.3 ← alloy 1.8.3 ← {recon, relays-client, searcher-rs, shared-rs, token-enricher}
```
El código propio **no construye signers de alloy**:

```
$ grep -rn 'PrivateKeySigner\|LocalWallet\|SignerMiddleware\|alloy_signer\|sign_transaction' backend/*/src --include=*.rs
  → 4 coincidencias en searcher-rs, TODAS comentarios («carries NO SignerMiddleware/Wallet/LocalWallet»,
    «NEVER wrap with SignerMiddleware / .with_signer», «no LocalWallet attached (capital_exposed == 0)»)
  → 0 usos reales de API de firma de alloy
```

### 4.2 ethers-signers (SÍ es camino de firma) — corrección a la hipótesis inicial

```
crypto-bigint 0.5.5 ← elliptic-curve 0.13.8 ← ethers-signers 2.0.14 ← {
  relays-client/src/signer.rs      (FLASHBOTS_SIGNER_KEY → LocalWallet)
  relays-client/src/relay_flashbots.rs (EPHEMERAL_STAGING_KEY → LocalWallet)
  relays-client/src/multi_relay.rs, bundle_builder.rs (.sign_transaction)
}
```
`ethers-rs` 2.0.14 es la **última release** (el proyecto está deprecado a favor de alloy) y fija
`elliptic-curve ^0.13`, así que **la migración a alloy 2 NO elimina esta cadena por sí sola**: hay que
migrar además la firma de `relays-client` de ethers-rs a alloy 2.

### 4.3 ¿Se ejecuta hoy?

| camino | gate | estado en producción |
|---|---|---|
| `bundle_builder.rs:57-58` → `LiveExecPolicy::from_env().assert_broadcast_allowed(chain)` → `.sign_transaction` (línea 188) | `live_exec_policy` (default-deny) | **NO se ejecuta**: `paper_mode=true`, policy default-deny |
| `relay_flashbots.rs:626-668` staging probe (§32) con `EPHEMERAL_STAGING_KEY` | key hardcodeada en el repo | se ejecuta con una clave **efímera y pública** (no es secreto del operador) |

Además el advisory es de **canal lateral temporal**: requiere un atacante co-localizado que mida
tiempos de operaciones dependientes de branches secretos. En el despliegue actual **no hay secretos
del operador procesados** (paper mode; sin broadcast; la única clave usada en el probe es efímera y
está en el código).

---

## 5. Plan de remediación (t24 ALLOY2) — alcance corregido

1. `cargo update` + migración a **alloy 2.x** (`alloy`, `alloy-primitives 2`, `alloy-sol-types 2`,
   `alloy-provider`, `alloy-network`): elimina la cadena de alloy-signer.
2. **Migrar la firma de `relays-client` de ethers-rs a alloy 2 signers** (ethers-signers 2.0.14 es EOL
   y fija elliptic-curve 0.13): sin este paso `crypto-bigint 0.5.5` sigue en el grafo.
3. Reemplazar `serde_yaml` (searcher-rs) por un parser mantenido (`serde-saphyr`/`saphyr`/`noyalib`) o
   justificar un ignore separado para 0449.

Hasta que (1)+(2) estén en main, el puente propuesto es un **ignore firmado**, no un allowlist permanente.

---

## 6. Entry propuesta (NO aplicada)

En `.github/workflows/security.yml`, job `cargo-audit`, variable `RUSTSEC_IGNORES`:

```yaml
# ── BRIDGE 2026-10-24 · operador: <firma> · revisión obligatoria 2026-11-07 ──
# RUSTSEC-2026-0387 crypto-bigint 0.5.5 (timing variability en ops constant-time).
#   Ruta: crypto-bigint 0.5.5 ← elliptic-curve 0.13.8 ← {ethers-signers 2.0.14 ← relays-client
#   signer.rs/relay_flashbots.rs} ∪ {alloy-signer 1.8.3 ← alloy 1.8.3}. Sin fix en semver
#   (cargo update -p crypto-bigint --precise 0.7.6 falla; alloy 1.8.3 es la última 1.x y
#   alloy-network la requiere no-opcional). Exposición: firma local; el camino de broadcast está
#   en default-deny (paper_mode + live_exec_policy); la clave del staging probe es efímera y pública;
#   el advisory exige observador co-localizado midiendo tiempos. Plan: t24 ALLOY2 (alloy 2 +
#   migrar la firma de relays-client de ethers-rs a alloy 2 signers).
# RUSTSEC-2024-0449 unsafe-libyaml 0.2.11 (unmaintained; sin parche).
#   Ruta: unsafe-libyaml ← serde_yaml 0.9.34+deprecated ← searcher-rs. Sin fix upstream; el reemplazo
#   por un parser YAML mantenido va en t24. `informational = "unmaintained"` (no es vulnerabilidad).
--ignore RUSTSEC-2026-0387 --ignore RUSTSEC-2024-0449
```

**Nota de mecánica de expiry (fail-honest):** el workflow hoy enforça **una sola** fecha global
(`ALLOWLIST_EXPIRES`, hoy `2030-12-31`) para todo `RUSTSEC_IGNORES`. Agregar estas dos ids bajo esa
variable las dejaría válidas hasta 2030, **no** hasta 2026-11-07. Para que la fecha propuesta sea
*máquina-enforzada* hace falta un cambio chico en el step de expiry (soporte de expiry por id) o un
recordatorio externo. Este documento propone la fecha 2026-11-07 como **revisión obligatoria
documentada**; si el operador quiere enforcement duro, se hace en el mismo PR del ignore con la
aprobación correspondiente (no se incluyó acá para no tocar el mecanismo del gate en el puente).

---

## 7. Reproducibilidad

```bash
cd backend
cargo audit                                   # 1 vulnerability (crypto-bigint) + 2 warnings
cargo update -p crypto-bigint --precise 0.7.6 # falla: requirement ^0.5 de elliptic-curve 0.13.8
cargo tree -i crypto-bigint                   # cadena completa
cargo tree -i unsafe-libyaml                  # serde_yaml ← searcher-rs
cargo tree -p ethers-signers --depth 1        # ethers-signers 2.0.14 → elliptic-curve 0.13.8
```
