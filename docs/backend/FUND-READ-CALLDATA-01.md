# FUND-READ-CALLDATA-01 — `balanceOf` calldata de 24 bytes en `sim-ctl`

**Estado:** fix + tests + rama + PR DRAFT. **Sin merge, sin deploy, sin escritura on-chain.**
**Archivo:** `backend/sim-ctl/src/signer_funding.rs` (único archivo de código tocado).
**Severidad:** la verificación del centinela medía OTRA cuenta — la escritura siempre fue correcta.

---

## 1. Revisión servida (base)

| Dato | Valor | Cómo se obtuvo |
| --- | --- | --- |
| Rama base | `main` | — |
| SHA base | `d1a4c3f5445917a5f5c361e79daf3351ef9f2983` | `git ls-remote https://github.com/hefarica/arbitragex-v2 refs/heads/main` |
| Commit base | `Merge pull request #864 from hefarica/sre/n05-n12-instruments-01` | `git log -1 --format='%H %ad %s'` |
| Archivo afectado | 656 líneas, 28978 bytes | `git rev-parse d1a4c3f5:backend/sim-ctl/src/signer_funding.rs` |

`main` **no** había avanzado respecto del SHA declarado en el encargo: se trabajó exactamente
sobre `d1a4c3f5`.

### 1.1 Integridad por bytes (`git hash-object`, blobs de git)

| Objeto | Blob SHA-1 |
| --- | --- |
| `backend/sim-ctl/src/signer_funding.rs` **ANTES** | `08821917590a707ba5d5651a39f21330beb19db2` |
| `backend/sim-ctl/src/signer_funding.rs` **DESPUÉS** | `6b5fb633f4cdd7547a8bb9ae31f2386cb4d2d734` |
| Mutante (fix revertido, ver §5.2) | `98d9c9816ef0920cedbbff670739c1448601cd64` |
| `backend/simulator-v2/src/sequence_runner.rs` (referencia, **NO tocado**) | `1f6cf43c7c7fdfac4a653c07c0a59312f0f17d3d` |

El blob de partida se verificó por triple vía y las tres coinciden: `git rev-parse d1a4c3f5:<ruta>`
== `git hash-object` del working tree del clon aislado == `git hash-object` de la copia leída en
crudo desde `raw.githubusercontent.com` a ese SHA. Ningún mtime ni re-encoding interviene: son
hashes de contenido.

> Nota de entorno: **nunca** se usó `Out-File`/`>` de PowerShell para verificar un hash. Todas las
> comparaciones son `git hash-object` sobre los bytes reales.

---

## 2. El defecto (medido)

`backend/sim-ctl/src/signer_funding.rs`, función `balance_of`, líneas 413-418 en `d1a4c3f5`:

```rust
let sel: [u8; 4] = [0x70, 0xa0, 0x82, 0x31]; // balanceOf(address)
let mut data = sel.to_vec();
data.extend_from_slice(signer.as_bytes());   // ← &[u8; 20] crudo
```

`signer` es un `H160` de `ethers`; `as_bytes()` devuelve `&[u8; 20]`. El calldata medía
**4 + 20 = 24 bytes** en vez de los **36** que exige el ABI.

**Por qué importa.** La EVM rellena con ceros más allá de `calldatasize`, así que el calldata
malformado **no falla**: se ejecuta y devuelve 32 bytes — de **otra cuenta**. `CALLDATALOAD(4)`
devuelve la palabra `signer[0..20] || 0^12` y `uint160(word)` conserva sus 20 bytes bajos,
es decir `signer[12..20] || 0^12`.

La escritura del centinela **siempre estuvo bien**: usa `ethers::abi::encode`, que emite una
palabra de 32 bytes con la dirección alineada a la derecha (el mismo mecanismo que
`balance_slot`, corregido en SIM-FUND-01b). Resultado: se escribía en el slot correcto y se
leía en **otra dirección**, así que el centinela no podía reproducirse nunca. El síntoma
—escritura correcta, verificación fallida— era indistinguible de un slot equivocado.

---

## 3. Evidencia on-chain (`cast call`, con número de bloque)

Bloque fijado: **`26146398`** (head leído con `cast block-number`; el bloque se pasó explícito con
`--block` en las cinco llamadas). Cadena: Ethereum mainnet (chain id 1).

- Token: WETH9 `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2`
- Titular: par UniswapV2 USDC/WETH `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc`
- Selector confirmado por herramienta: `cast sig "balanceOf(address)"` → `0x70a08231`
- `cast Version: 1.7.2-nightly` (SHA `c5e44b5e45a5872f1eb213714555e94725c94c78`)

Calldata usados (longitudes verificadas en el propio comando):

```text
36 B : 0x70a08231000000000000000000000000b4e16d0168e52d35cacd2c6185b44281ec28c9dc
24 B : 0x70a08231b4e16d0168e52d35cacd2c6185b44281ec28c9dc
36 B : 0x70a0823100000000000000000000000085b44281ec28c9dc000000000000000000000000
```

### Salida cruda

```text
HEAD_NOW=26146401
PINNED_BLOCK=26146398
selector=0x70a08231
CALLDATA_36B_LEN=36  hex=0x70a08231000000000000000000000000b4e16d0168e52d35cacd2c6185b44281ec28c9dc
CALLDATA_24B_LEN=24  hex=0x70a08231b4e16d0168e52d35cacd2c6185b44281ec28c9dc
CALLDATA_36B_MISREAD_LEN=36  hex=0x70a0823100000000000000000000000085b44281ec28c9dc000000000000000000000000

=== [1] 36 B on WETH9 balanceOf(pair) @ block 26146398 (publicnode) ===
0x0000000000000000000000000000000000000000000000d85e8fb1e6ad0deb8a
=== [2] 24 B on WETH9 balanceOf(pair) @ block 26146398 (publicnode) ===
0x0000000000000000000000000000000000000000000000000000000000000000
=== [3] 36 B on WETH9 balanceOf(0x85b44281ec28c9dc000000000000000000000000) @ block 26146398 (publicnode) ===
0x0000000000000000000000000000000000000000000000000000000000000000
=== [4] CROSS-PROVIDER: 36 B @ block 26146398 (drpc) ===
0x0000000000000000000000000000000000000000000000d85e8fb1e6ad0deb8a
=== [5] CROSS-PROVIDER: 24 B @ block 26146398 (drpc) ===
0x0000000000000000000000000000000000000000000000000000000000000000
```

RPCs: `https://ethereum-rpc.publicnode.com` y `https://eth.drpc.org`. Dos proveedores
independientes devuelven **el mismo valor byte a byte** en el mismo bloque.
(`https://eth.llamarpc.com` estaba caído — HTTP 525 — y no se usó.)

### Las tres identidades que importan

| Medición | Resultado | Naturaleza |
| --- | --- | --- |
| `36B(pair)` @ 26146398 | `0x…d85e8fb1e6ad0deb8a` = 3991310580286801963914 wei = 3991.310580286801963914 WETH | **depende del bloque** — se cita con su bloque, no como constante |
| `24B(pair)` @ 26146398 | `0x0000…0000` | determinista |
| `36B(0x85b44281ec28c9dc000000000000000000000000)` @ 26146398 | `0x0000…0000` | determinista |

La identidad queda demostrada por igualdad de salidas **en el mismo bloque**:

```text
24B(pair)  ==  36B(0x85b44281ec28c9dc000000000000000000000000)  ==  0x0
```

y `0x85b44281ec28c9dc000000000000000000000000` es exactamente `pair[12..20] || 0^12`:
`pair = b4e16d0168e52d35cacd2c61 85b44281ec28c9dc`, luego sus 8 bytes finales
(`85b44281ec28c9dc`) encabezan la dirección que la EVM leía de verdad.

**No se hardcodea el saldo de 36 B**: ese valor es estado, no una constante, y así se reporta.

---

## 4. El arreglo

Reemplazo en `balance_of` (una línea) + un encoder puro y testeable, espejo del canónico que ya
existía en el repo (`simulator-v2/src/sequence_runner.rs:717`), que está FUERA de alcance y no se
modificó.

```rust
/// Canonical ERC-20 `balanceOf(address)` selector. `cast sig
/// "balanceOf(address)"` == `0x70a08231`.
const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];

/// ABI-required calldata length: 4-byte selector + ONE 32-byte argument word.
const BALANCE_OF_CALLDATA_LEN: usize = 36;

fn build_balance_of_calldata(signer: Address) -> Vec<u8> {
    // Idiomatic: let ethers' ABI encoder do the padding — the same mechanism
    // `balance_slot` below relies on. `abi::encode` right-aligns an address
    // inside its word, which is exactly what the EVM reads back.
    let mut data: Vec<u8> = Vec::with_capacity(BALANCE_OF_CALLDATA_LEN);
    data.extend_from_slice(&BALANCE_OF_SELECTOR);
    data.extend_from_slice(&ethers::abi::encode(&[ethers::abi::Token::Address(signer)]));
    debug_assert_eq!(data.len(), BALANCE_OF_CALLDATA_LEN);
    data
}
```

y en `balance_of`:

```rust
-    let sel: [u8; 4] = [0x70, 0xa0, 0x82, 0x31]; // balanceOf(address)
-    let mut data = sel.to_vec();
-    data.extend_from_slice(signer.as_bytes());
+    let data = build_balance_of_calldata(signer);
```

### Calldata ANTES y DESPUÉS (36 bytes, medido)

```text
ANTES  : 0x70a08231 b4e16d0168e52d35cacd2c6185b44281ec28c9dc                         (24 B)
DESPUÉS: 0x70a08231 000000000000000000000000 b4e16d0168e52d35cacd2c6185b44281ec28c9dc  (36 B)
```

---

## 5. Tests — y la prueba de que son FALSIFICADORES

Tres tests nuevos en el `mod tests` del propio archivo:

| Test | Qué ancla |
| --- | --- |
| `build_balance_of_calldata_is_36_bytes_with_a_right_aligned_signer` | LONGITUD == 36 y BYTES == selector + 12 ceros + 20 de la dirección, más el hex completo pineado |
| `build_balance_of_calldata_differs_per_signer` | dos cuentas → calldata distinto |
| `balance_of_calldata_bidirectional_control_on_mainnet_vectors` | **bidireccional**: el MISMO verificador acepta el calldata de 36 B y rechaza el de 24 B, y muestra que el de 24 B lee `0x85b44281ec28c9dc000000000000000000000000` |

### 5.1 Gates verdes sobre el blob definitivo `6b5fb633`

```text
=== 0. TOOLCHAIN ===
/home/hfrc/.cargo/bin/cargo
cargo:   cargo 1.91.0 (ea2d97820 2025-10-10)
rustc:   rustc 1.91.0 (f8297e351 2025-10-28)
rustfmt: rustfmt 1.8.0-stable (f8297e351a 2025-10-28)

=== 7. cargo fmt --all -- --check (0 bytes == zero diffs) ===
fmt_check_exit=0
fmt_check_bytes=0

=== 8. cargo test -p sim-ctl --no-fail-fast ===
cargo_test_exit=0
Compiling=1
Checking=0
Fresh=0
FIRST_COMPILE_LINE:    Compiling sim-ctl v0.1.0 (/home/hfrc/arbx-fund01/backend/sim-ctl)

=== 9. TEST TALLIES ===
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 72 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.50s
test result: ok. 8 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

=== 10. OUR TESTS (FUND-READ-CALLDATA-01) ===
test signer_funding::tests::build_balance_of_calldata_differs_per_signer ... ok
test signer_funding::tests::balance_of_calldata_bidirectional_control_on_mainnet_vectors ... ok
test signer_funding::tests::build_balance_of_calldata_is_36_bytes_with_a_right_aligned_signer ... ok
```

Total: **105 passed / 0 failed / 2 ignored** en 5 targets (`src/lib.rs`, `src/main.rs`,
`tests/simwire02_pel_recovery.rs`, `tests/simwire02_route_aware.rs`,
`tests/simwire02c_redelivery_idempotency.rs`). Los 2 `ignored` son preexistentes.

En el primer build del clon limpio la salida mostró **526 líneas `Compiling` y 0 `Fresh`**
(primer crate compilado: `proc-macro2 v1.0.107`); en la corrida sobre el blob definitivo
**1 `Compiling`: `sim-ctl` mismo** — el crate bajo test se recompiló desde la fuente modificada.
**Nunca** un `Fresh` sobre fuente modificada.

### 5.1.1 `cargo clippy -p sim-ctl --all-targets`

```text
clippy_exit=0
clippy_error_count=0
--- warnings mentioning OUR file ---
0
--- all warning headers (raw) ---
2:warning: the use of negated comparison operators on partially ordered types produces code that is hard to read and refactor, ...
11:warning: `assert!(true)` will be optimized out by the compiler
24:warning: `assert!(true)` will be optimized out by the compiler
33:warning: `sim-ctl` (bin "sim-ctl" test) generated 3 warnings
```

Las 3 advertencias restantes son **preexistentes** y viven en `src/consumer.rs`; **ninguna**
menciona `signer_funding.rs`. Un warning `clippy::redundant_slicing` que este mismo cambio había
introducido en una aserción de test se eliminó **antes** del commit (por eso el blob final es
`6b5fb633`): el cambio no agrega ni un warning nuevo.

### 5.2 Prueba de falsación: revertir el fix pone los tests en ROJO

Se mutó el encoder de vuelta al append de 24 bytes **sobre el blob final** (blob mutante
`98d9c9816ef0920cedbbff670739c1448601cd64`) y se corrieron los tests:

```text
original:     data.extend_from_slice(&ethers::abi::encode(&[ethers::abi::Token::Address(signer)]));
mutated:      data.extend_from_slice(signer.as_bytes()); // MUTATION: reverted fix
mutated_cargo_exit=101
test signer_funding::tests::balance_of_calldata_bidirectional_control_on_mainnet_vectors ... FAILED
test signer_funding::tests::build_balance_of_calldata_is_36_bytes_with_a_right_aligned_signer ... FAILED
test signer_funding::tests::build_balance_of_calldata_differs_per_signer ... FAILED
test result: FAILED. 10 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

thread '…' panicked at sim-ctl/src/signer_funding.rs:434:5:
```

Restaurado el archivo: `restored_blob=6b5fb633f4cdd7547a8bb9ae31f2386cb4d2d734` y
`restored_cargo_exit=0` (13 passed / 0 failed). **Verde → mutar → rojo → restaurar → verde.**
Los tests dependen del código, no de sí mismos.

---

## 6. Alcance del defecto (auditoría del patrón)

Búsqueda del patrón en TODO el repo (`*.rs`) por selector y por literal `balanceOf`:

| Ubicación | Estado |
| --- | --- |
| `backend/sim-ctl/src/signer_funding.rs:413` | **DEFECTUOSO** — calldata de 24 B (este fix) |
| `backend/simulator-v2/src/sequence_runner.rs:547` | correcto (36 B, padding explícito) |
| `backend/simulator-v2/src/sequence_runner.rs:717` | correcto — referencia canónica |
| `backend/prioritization-spine/src/swap_encoder.rs:95` | correcto (`encode(&[Token::Address])` + `prepend_selector`) |
| `backend/prioritization-spine/src/round_trip_executor.rs:567,571` | consume el encoder correcto de arriba |

**Un solo sitio defectuoso.** `sequence_runner.rs` estaba fuera de alcance y no se tocó.

---

## 7. Entorno de verificación (y sus trampas, medidas)

- **Windows: `cargo` está bloqueado por Smart App Control (`os error 4551`).** No se intentó
  desactivar. La ruta usada es **WSL2 Ubuntu** con la toolchain que fija el propio repo
  (`rust-toolchain.toml`, channel `1.91.0`, componentes `rustfmt` + `clippy`).
- **`ssh arbx` no sirve para esto:** `command -v cargo` devuelve vacío incluso con login shell
  (`bash -lc`). Dato medido, no supuesto → esa ruta quedó descartada y se declaró.
- **Trampa 127 / log vacío:** el wrapper de gates ejecuta `command -v cargo` como PRIMERA línea y
  aborta con 127. En el primer intento falló de verdad (shell no-login) y el guard lo cazó en vez
  de leer un log vacío como "0 errores". Se declara: los gates corren con `bash -l`.
- **El workspace NO está en la raíz del repo.** `cargo` desde la raíz falla con
  ``could not find `Cargo.toml` in `/home/hfrc/arbx-fund01` ``. La raíz es `backend/`. Todos los
  comandos corren desde `~/arbx-fund01/backend`.
- **Disco:** nunca se corrió `cargo test --workspace`. Solo `-p sim-ctl`. `/` pasó de 91 GB a
  95 GB usados durante el build (861 GB libres al cierre).
- Clon aislado en `~/arbx-fund01` (WSL) y en `%TEMP%\arbx-fund01` (Windows, para git/PR).
  **El checkout compartido no se usó en ningún momento**: una línea leída de ahí no aplica.

---

## 8. Qué NO se hizo (límites respetados)

- **No** merge. **No** deploy. **No** escritura on-chain, firma ni broadcast: las llamadas de §3
  son `eth_call` de lectura pura.
- **No** se tocó `CANDIDATE_SLOTS`, ningún umbral de rentabilidad, el gas, el sizing ni el gate
  de paper.
- **No** se tocó Redis ni ninguna configuración.
- **No** se modificó `sequence_runner.rs` ni ningún otro archivo: **1 archivo**, sin reformateo
  ajeno (verificado con `git status --porcelain` tras `cargo fmt --all`).
- **No** se aplicó el fix a los otros call sites porque **no** tienen el defecto (§6).
