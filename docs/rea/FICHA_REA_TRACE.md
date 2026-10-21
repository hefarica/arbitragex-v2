# FICHA REA-trace — REVERT-TRACE-01: revert con address + pc + opcode

Fuentes fijadas:
- `paradigmxyz/artemis@4bb158070833ec2b789a2ece14c896c4b28ce3be` (HEAD `main`, 2026;
  dual MIT/Apache-2.0 — `LICENSE-MIT`/`LICENSE-APACHE` en la raíz del árbol).
- `flashbots/mev-inspect-rs@b6e2f0a4784d063375734e2773fa7196cf43174d` (HEAD `master`,
  REPO ARCHIVADO — uso declarado: referencia de semántica, Tier 3, su actividad no puntúa).
- Hecho canónico citable: documentación de Solidity, sección
  "Panic via `assert` and Error via `require`"
  (https://docs.soliditylang.org/en/latest/control-structures.html) — tabla de códigos
  `Panic(uint256)` y el hecho `revert` = instrucción `0xfd`.

## 0. Estado de gate (rea_adopt, corridos para el destino `backend/simulator-v2/src/sequence_runner.rs`)

| Fuente | Veredicto | Detalle |
|---|---|---|
| `flashbots/mev-inspect-rs@b6e2f0a` | **BLOQUEADO para código**, `modo_permitido: solo_vectores_numericos` | Sin licencia en el sha + fuera de allowlist. CONSISTENTE con su rol declarado Tier 3: semántica de referencia, cero código. Enmienda (solo si algún día entrara código): `CONFIRMED: allowlist amendment flashbots/mev-inspect-rs`. |
| `paradigmxyz/artemis@4bb1580` | **GO_VERBATIM** por gates (MIT + allowlist Tier 2 + `CONFIRMED: artemis patterns`) **pero el segmento NO EXISTE** | El adopt aprueba los gates y no hay nada que portar: el descarte queda documentado con evidencia de árbol (§1.1). |
| Destino `sequence_runner.rs` | `campos_faltantes: tier_fusile` | El destino no mapea a ninguna familia conocida de la allowlist (familia nueva: simulador). El operador decide tier si este destino se somete a ports formales. |

Esta ficha trabaja en modo semántica + números: cero líneas de código ajeno.

## 1. Evaluación de candidatos (fail-honest)

### 1.1 artemis@4bb1580 — DESCARTADO como fuente del segmento

Hipótesis del encargo: "artemis inspector/decodificación". Verificación al sha fijado
(árbol completo vía GitHub API, `git/trees/4bb1580?recursive=1`):

- `crates/` = `artemis-core`, `clients`, `generator`, `strategies` (única raíz de código Rust).
- Scan recursivo de rutas con `inspect|trace|revert|evm|decode|error`: los ÚNICOS matches
  son ABIs de errores **vendoreadas en contratos Solidity de estrategias** (p.ej.
  `crates/strategies/opensea-sudo-arb/bindings/src/reentrancy_errors.rs`,
  `contracts/lib/forge-std/src/StdError.sol`) — bindings de `abigen`, no un inspector.
- No existe ningún módulo Rust que decodifique reverts a frames (address/pc/opcode).

⇒ No hay segmento que portar. artemis sigue siendo válido para SU uso allowlistado
(`backend/searcher-rs`, patrones collector→strategy→executor), no para REVERT-TRACE-01.

### 1.2 mev-inspect-rs@b6e2f0a — referencia de SEMÁNTICA de frame (Tier 3, archivado)

Qué aporta y qué no (leído `src/main.rs`, `src/traits.rs`, `src/types/inspection.rs` al sha):

- **Modelo de traza: parity/openethereum** (`provider.trace_transaction` /
  `trace_block` — `main.rs`, comando `Tx` y `process_block`). El frame parity es
  `{action: Call{from, to, call_type, gas, value, input}, error: Option<String>,
  trace_address, subtraces}`.
- **Status del tx**: `types/inspection.rs` — `if trace.error.is_some() {
  inspection.status = Status::Reverted; }` (bloque `TryFrom<TraceWrapper>` para
  `Inspection`), con el comentario honesto del propio upstream: *"Revert if all
  subtraces revert? There are counterexamples e.g. when a low-level trace's revert
  is handled"* — un revert de subllamada MANEJADO por un `try/catch`/low-level call
  NO implica que el tx revirtió.
- **Address del frame fallido**: `call.to` del frame cuyo `error` está seteado
  (semántica parity). **Proxy**: si el primer frame es `DelegateCall` desde
  `inspection.contract`, `proxy_impl = Some(call.to)` — distingue el proxy de la
  lógica.
- **LO QUE NO APORTA**: `pc` y `opcode`. Las traces parity NO los llevan; viven en
  los structLogs de geth (`debug_traceTransaction`), que mev-inspect-rs no consume.
  Y está archivado (último push 2022) — su actividad no puntúa, per encargo.

## 2. Semántica del decode de revert data (dispatch canónico)

Fuente del hecho: Solidity docs (URL arriba). Selectores como hechos citables:

| Selector | Tipo | Semántica |
|---|---|---|
| `0x08c379a0` | `Error(string)` | `require(cond, "msg")`, `revert("msg")`, etc. ABI: offset `0x20` + len + UTF-8. |
| `0x4e487b71` | `Panic(uint256)` | `assert` + panics del compilador. Payload exacto: selector + 1 word ABI (4+32 = **36 bytes**). |
| (4 bytes, otro) | custom error | `revert CustomError(...)`. **Sin ABI del contrato NO es nombrable** — solo se reporta el selector. |
| (vacío) | — | `require(bool)` sin mensaje y `revert()` revierten **sin ningún dato, ni siquiera selector**. |

Códigos `Panic` canónicos (Solidity docs, lista textual):

```
0x00 generic compiler panic   0x01 assert false
0x11 arithmetic under/overflow (checked)     0x12 division or modulo by zero
0x21 enum conversion out of range            0x22 storage byte array incorrectly encoded
0x31 .pop() on empty array                   0x32 array/bytesN index out of bounds
0x41 too much memory allocated               0x51 zero-initialized internal function called
```

Además (hechos citables de la misma fuente): internamente Solidity revierte con la
instrucción `0xfd` (`REVERT`); antes de 0.8.0 los Panic usaban `invalid` (`0xfe`) y
consumían todo el gas.

### 2.1 Regla de compatibilidad del destino (hallazgo del vector T5b)

`deadbeef` (4 bytes) es un selector custom VÁLIDO y a la vez datos crudos: la
ambigüedad no es resoluble sin ABI. El test existente del destino
(`sequence_runner.rs:1507-1512`) espera display `"0xdeadbeef"` (fallback hex).
⇒ Contrato del port: **el string de display NO cambia** (fallback hex, cap 36 bytes,
regla PANIC-CODE-01); la clasificación viaja en un `kind` ESTRUCTURADO aparte
(`ErrorString | Panic(code, named) | Selector(0x…) | Empty | Raw`).

## 3. Formato de traza que hay que emitir

Basado en el `OriginFrame` que YA existe en el destino (feature `revert-trace`,
`sequence_runner.rs:318-331`), extendido con `kind`:

```rust
pub struct RevertFrame {
    pub address: String,      // executing code address del frame que reverteó
                              // (revm: inputs.bytecode_address en call_end)
    pub depth: usize,         // 0 = top-level
    pub pc: usize,            // pc de la ÚLTIMA instrucción antes del revert (hook step)
    pub opcode: u8,           // opcode en ese pc — típicamente 0xFD (REVERT) o 0xFE (INVALID)
    pub kind: RevertKind,     // ErrorString(String) | Panic{code, named} | Selector([u8;4]) | Empty | Raw
    pub data_hex: String,     // payload crudo (hex), para diagnósticos
    pub gas_used: u64,
    pub seq: usize,           // orden de aparición (innermost-first)
}
```

Reglas de atribución (decisión ya tomada por el destino, `sequence_runner.rs:287-293`,
y CONSISTENTE con la advertencia de mev-inspect-rs §1.2):

1. Los frames se desenvuelven **innermost-first**: el PRIMER `call_end`/`create_end`
   con `is_revert()` es el ORIGEN — un `Panic(uint256)` se atribuye al contrato que
   PANIQUEÓ, no al `catch` que lo re-codificó más arriba.
2. Un revert de subllamada **manejado** (low-level call que devuelve false, try/catch)
   también dispara `call_end` con revert: hay que marcar `handled` vs `fatal`
   (fatal = el outcome del TOP-LEVEL es revert). El `origin()` del tracer actual no
   distingue — borde a cubrir en el PR.
3. `pc`/`opcode` salen del hook `step` (última instrucción vista del frame);
   address del hook `call_end` (`inputs.bytecode_address`) — mecanismo ya
   implementado por `RevertTracer` con revm 42 (`Interpreter::bytecode.pc()`,
   `.opcode()`, `input.target_address()`).

## 4. Mapa al destino — dónde se pierde el frame HOY

Cadena productiva actual (todas las rutas verificadas en el árbol base HEAD `f27bdb33`):

| Punto | Qué pasa | Referencia |
|---|---|---|
| `evm.transact(tx)` sin inspector | revm 42 ejecuta; el `ExecutionResult::Revert` solo trae el outcome TOP-LEVEL (gas + output). Sin `Inspector` montado no hay frames internos. | `backend/simulator-v2/src/revm_runner.rs:88-116` |
| Aplanado a String | `SimError::Reverted(reason)` — `decode_revert_reason` solo decodifica `Error(string)`; Panic queda hex-crudo; custom sin identificar. | `revm_runner.rs:106-115`, `:232-251` |
| Outcomes de secuencia | `DeployOutcome::Reverted { gas_used, reason }` / `CallOutcome::Reverted { gas_used, reason }` — sin address/pc/opcode. | `sequence_runner.rs:152`, `:180`, `:719/:726`, `:795/:802` |
| Decode duplicado | `decode_revert_reason` de `sequence_runner.rs:1055-1069` (PANIC-CODE-01: cap 36 bytes; medido en G-SIM-1: 15/21 rutas llevaban `0x4e487b71` y el código del panic se perdía). | `sequence_runner.rs:1044-1069` |
| Tracer YA EXISTE pero test-only | `RevertTracer` (SIM-REVERT-TRACE-01) captura EXACTAMENTE address+depth+pc+opcode+stack+data y emite `VARIANCE_BENCH_REVERT_FRAME={json}` — tras `#[cfg(feature = "revert-trace")]`, compilado FUERA de producción, salida por `println!` que no viaja en ningún outcome. | `sequence_runner.rs:287-472` |
| Consumidores downstream | counters tag `revm_reverted` (solo reason), orchestrator `RevmReverted{reason}`, relays-client persistence mapea `ExecutionStatus::Reverted` → string `"failed"`/`"reverted"`. | `searcher-rs/src/counters.rs:295`, `searcher-rs/src/sim_orchestrator.rs:136-166`, `relays-client/src/persistence.rs:49`, `:119` |

El gap del PR REVERT-TRACE-01 queda así delimitado (el mecanismo existe; falta
productivizarlo):

1. `RevertFrame` como campo `Option<RevertFrame>` (R8: `None` = no computado) en
   `CallOutcome::Reverted`, `DeployOutcome::Reverted` y `SimError::Reverted`.
2. Inspector montado en el camino productivo (el `NoopInspector` ya existe como
   placeholder de costo cero, `sequence_runner.rs:266-272`): capturar SOLO en revert,
   sin snapshot de stack por paso (el stack top del hook `step` ya se guarda en
   `StepPoint` — mantener las 12 words del top, útiles para el operando del panic).
3. Dispatch de decode §2 + `kind` estructurado (display invariante, §2.1).
4. Distinguir `handled` vs `fatal` (regla 2 de §3).
5. Propagar el frame hasta persistencia/observabilidad (hoy `reverted` es un string).
6. G5: `sequence_runner.rs`/`revm_runner.rs` NO son rutas protegidas — limpio.

## 5. Vectores

**Vector upstream con address+pc+opcode: NO EXISTE en los candidatos** — declarado con
razón: las traces parity de mev-inspect-rs no llevan pc/opcode (§1.2) y artemis no tiene
el segmento (§1.1). Por regla del encargo, el vector es DERIVADO y queda marcado así.

### V-derivado — dispatch de decode (bytes exactos + nombres)

Artefacto: `docs/rea/verifica_vectores_rea_trace.py` (determinista, stdlib, sin red) —
salida `docs/rea/verifica_vectores_rea_trace.log` (22/22 OK, exit 0). Items:

| # | Payload (hex) | Display esperado | Kind esperado |
|---|---|---|---|
| T1 | `08c379a0` + offset `0x20` + len + `"insufficient_balance"` | `Error(string): insufficient_balance` | `ErrorString` |
| T2 | `4e487b71` + 31×`00` + `11` (36 bytes exactos) | `Panic(0x11): arithmetic underflow/overflow` | `Panic(0x11)[named]` |
| T3 | los 10 códigos canónicos `0x00..0x51` + `0x99` desconocido | nombre canónico / `unknown code` | `Panic(code)` |
| T4 | `2ac11621` (custom, 4 bytes) | `0x2ac11621` (fallback hex INVARIANTE) | `Selector(0x2ac11621)` |
| T5a | vacío | `empty` | `Empty` |
| T5b | `deadbeef` | `0xdeadbeef` (compat test destino `sequence_runner.rs:1508-1511`) | `Selector(0xdeadbeef)` |
| T5c | `deadbe` (<4 bytes) | `0xdeadbe` | `Raw` |
| T6 | — | hecho citable: `REVERT = 0xfd` (Solidity docs) | — |

### V-baseline — tests existentes del destino que el PR NO puede romper

`sequence_runner.rs:1486-1531`: `decode_revert_reason_parses_error_string` (msg
`"insufficient_balance"`), `decode_revert_reason_hex_fallback` (`0xdeadbeef`),
`decode_revert_reason_keeps_the_panic_code` (payload Panic(0x11) de 36 bytes, el
reason sobrevive con 2+72 chars hex). El display del port debe seguir pasándolos
byte-exacto.

### V-destino-medido — hecho real citado

G-SIM-1 (variance harness): **15 de 21 rutas** llevaban `multistep_call_revert`
con `0x4e487b71` + 28 ceros cuando el cap era 32 bytes (PANIC-CODE-01,
`sequence_runner.rs:1048-1053`) — evidencia de que el panic-código se perdía en
producción de diagnóstico: la motivación medida del PR.

## 6. Checklist para `rea_verify` del PR REVERT-TRACE-01

- [ ] P1 citas `repo@sha` (mev-inspect-rs semántica + artemis descarte) en PR — artefacto: diff/PR body.
- [ ] P2 vector derivado T1-T6 transcrito a tests del destino con bytes literales — artefacto: `cargo test` verde.
- [ ] P3 baseline §5 intacto (los 3 tests existentes pasan sin editar) — artefacto: `cargo test`.
- [ ] P4 `RevertFrame` en outcomes productivos (no solo feature bench) — artefacto: tipos en diff + test que afirma `Some(frame)` con address/pc/opcode en un revert de fixture.
- [ ] P5 handled-vs-fatal cubierto por test (subllamada revertida manejada NO marca fatal) — artefacto: test.
- [ ] P6 display invariante (`0xdeadbeef` y cap 36) — artefacto: tests T4/T5b.
- [ ] P7 rutas protegidas intactas (`configs/app.toml`, `live_exec_policy.rs`, `bundle_builder.rs` broadcast) — artefacto: diff vacío.
- [ ] A1 zero-mock: fixtures de revert locales con bytes literales, sin pools on-chain.
- [ ] A2 no-hardcode: ninguna address/RPC/endpoint del fuente (el `FILTER`/`DYDX`/`ZEROX` de mev-inspect-rs NO entran — son su addressbook).
- [ ] A3 license: cero verbatim de mev-inspect-rs; artemis sin código portado (segmento inexistente).
