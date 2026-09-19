# 19. TESTING, FUZZING E INVARIANTES

CUÁNDO CARGAR ESTA REFERENCIA: vas a escribir o ampliar tests del motor Rust o de los contratos
Foundry; escribes o amplías tests del control-plane TS (frontend/edge worker/shared-ts): contratos
de API zod contra fixtures grabados, replay WS del dashboard, componentes o E2E Playwright; defines
property/invariant tests; montas un harness de simulación/replay o una suite de golden blocks;
diagnosticas divergencia sim-vs-chain; decides qué tests bloquean merge y qué corre nightly;
persigues flakiness en fork tests o en la suite vitest; auditas si la "cobertura" del repo mide
algo real.

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Pirámide L1-L6 | unit math → fuzz/property → invariant on-chain → fork pineado → shadow/paper → canary | cada capa atrapa la clase de bug que la anterior no puede ver |
| Fuzz Solidity dirigido | `forge test` (`runs`, `seed`, `max-anchor-time`) + `vm.assume` / `bound` (forge-std) | `bound` re-mapea el input (0 rechazos); `vm.assume` filtra y quema `max-test-rejects` |
| Invariantes on-chain | invariant testing con handler contracts + ghost variables | `targetContract`/`targetSelector` acotan el espacio; reverts con allowlist de razones |
| Fork determinista | `vm.createSelectFork(url, blockNumber)`, `vm.rollFork`, `vm.transact` | pinear + RPC archive = cero flakiness; fork al tip = ruleta rusa |
| Differential testing | implementación propia vs QuoterV2/SDK canónico + generated-table+probe | los vectores provienen SOLO de la referencia externa; regenerar = PR aparte |
| Property testing Rust | proptest + strategies para U256/decimales; criterion; miri | monotonía, conservación, idempotencia; fallos persisten en `proptest-regressions` |
| Fuzzing continuo | echidna / medusa (nightly, corpus persistente) + slither (gate de merge) | el corpus acumula secuencias adversarias para siempre |
| Replay histórico | revm/anvil con fork pineado; divergencia sim-vs-chain | suite de golden-blocks: bloques donde debió/no-debió detectar |
| CI gating | merge: unit + fuzz corto + fork core; nightly: deep fuzz + replay largo | presupuesto de tiempo explícito por job; nada de "verde a veces" |
| Gas como aserción | `forge snapshot --diff`, `forge test --gas-report`, `vm.pauseGasMetering` | una regresión de gas ES una regresión de PnL (ver núcleo §1.2) |
| Anti-flaky | semilla fija, reloj fakeado, sin red real bajo L4 | un test flaky es un test muerto; retry-until-green es mentira estadística |
| Contrato API TS | fixture grabado de producción (manifest bytes+sha256) parseado con el MISMO schema zod del cliente | wire↔schema drift rompe el test; prohibido editar el schema para pasar sin reproducir la respuesta real (RULE 00) |
| Replay WS determinista | secuencia grabada del bus (Redis Streams / socket.io) + reloj fakeado (`vi.useFakeTimers`/`vi.setSystemTime`) + golden de render | cero `Date.now`/`Math.random` sin semilla: no determinismo = flakiness garantizada (19.8, 19.9.2) |
| Pirámide TS | unit → contrato (zod vs fixtures) → componentes (render con datos fijos) → E2E Playwright | espejo de L1-L6 sin fork ni broadcast; sin lockfile pineado el CI no es reproducible |

## 19.1 La pirámide de tests de un motor de arbitraje

El núcleo §10 lista "todos los tests pasan" como checklist pre-deploy; esta referencia define qué
significa "todos" y en qué orden fallan. Seis capas, cada una más cara y más realista que la
anterior:

```
             ┌────────────────────────────────────────────┐
   L6        │ CANARY (capital mínimo, gated CLAUDE.md    │  broadcast real acotado,
             │ §34.5)                                     │  telemetría obsesiva
             ├────────────────────────────────────────────┤
   L5        │ SHADOW / PAPER en producción               │  pipeline real, ledger
             │ (detección + sim + decisión, 0 broadcast)  │  simulado (núcleo §1.2)
             ├────────────────────────────────────────────┤
   L4        │ FORK con estado real PINEADO               │  anvil/revm + bytecode real
             │ (anvil fork / vm.transact)                 │  de pools y routers
             ├────────────────────────────────────────────┤
   L3        │ INVARIANT on-chain (handlers)              │  secuencias aleatorias de
             │                                            │  estado sobre el contrato
             ├────────────────────────────────────────────┤
   L2        │ FUZZ / PROPERTY                            │  rangos numéricos que
             │ (forge fuzz + proptest)                    │  nadie escribió a mano
             ├────────────────────────────────────────────┤
   L1        │ UNIT MATH (puro, milisegundos)             │  constantes de pool, sqrt,
             │                                            │  redondeo, unidades wei
             └────────────────────────────────────────────┘
   costo y latencia por test aumentan hacia arriba ▲   velocidad de feedback hacia abajo ▼
```

| Capa | Atrapa | No puede atrapar |
|---|---|---|
| L1 | error de fórmula, overflow, redondeo, unidades (18 vs 6 decimales, wei vs ether) | interacción entre módulos |
| L2 | bordes numéricos (reserva de 1 wei, `amountIn` máximo, fee tiers raros), violaciones de propiedad en rangos amplios | estado compartido y secuencias |
| L3 | corrupción por secuencia de llamadas (doble ejecución de ruta, bypass de `paused`, fuga de fondos) | bugs de integración con bytecode real |
| L4 | quirks de protocolo (fee-on-transfer, transfer fees, routers no estándar), gas real, orden del bloque | drift del pipeline end-to-end (datos, Redis, PG) |
| L5 | decisiones correctas bajo datos y carga reales; honestidad R8 del pipeline completo | todo lo que exige broadcast real |
| L6 | latencia de relay, inclusión real, comportamiento de builders — el residuo irreducible | cualquier cosa a escala |

Reglas de la pirámide: (1) ningún bug que la capa N pudo atrapar barato debe llegar a N+1 — si
llegó, la capa N tiene un hueco que se cierra ANTES de seguir; (2) cada incidente en producción
debe aterrizar como test en la capa MÁS BAJA que lo reproduzca; (3) L5/L6 no sustituyen L1-L4:
son la única forma de ver el mundo real, pero la forma más cara de descubrir un error de fórmula.

## 19.2 Foundry en profundidad

### 19.2.1 Fuzzing dirigido: `runs`, `seed` y el binomio `vm.assume`/`bound`

Config base en `foundry.toml` (reproducible por defecto, no al revés):

```toml
[fuzz]
runs = 512
seed = "0xa11ce"          # CI fija la semilla; nightly puede aleatorizar
max-test-rejects = 65536  # presupuesto de inputs rechazados por vm.assume
max-anchor-time = 4000    # ms que Foundry dedica a anclar el snapshot de gas del fuzz

[invariant]
runs = 256
depth = 500               # llamadas por secuencia
fail-on-revert = false    # handlers own the revert policy (ver 19.2.2)
call-override = false
shrink-run = true         # minimiza el contraejemplo antes de reportar
```

Override por test sin tocar el archivo global (comentario inline, sintaxis exacta de Foundry):

```solidity
/// forge-config: default.fuzz.runs = 5000
function test_quote_extreme_ranges(uint128 amountIn) public { ... }
```

`vm.assume(cond)` **rechaza** el input y pide otro; si el dominio útil es pequeño (ej: fee tiers),
el 99% de los casos se rechaza y el test corre con basura o muere por `max-test-rejects`.
`bound(x, min, max)` (forge-std, `StdUtils`) **re-mapea** el input aleatorio al rango sin rechazar
ninguna corrida. Regla práctica: rangos continuos → `bound`; conjuntos discretos → `vm.assume`
sobre `x % n`, o mejor, seleccionar el elemento con un índice acotado.

```solidity
import {Test} from "forge-std/Test.sol";

contract SwapMathFuzz is Test {
    // Monotonía: más input NUNCA produce menos output en un swap sano.
    function test_getAmountOut_monotonic(uint128 x, uint112 reserveIn) public {
        x = uint128(bound(uint256(x), 1, 1e30));
        reserveIn = uint112(bound(uint256(reserveIn), 1, 1e24));
        uint256 out1 = SwapMath.getAmountOut(x, reserveIn, 1e18);
        uint256 out2 = SwapMath.getAmountOut(x + 1, reserveIn, 1e18);
        assertGe(out2, out1, "monotonicity violated");
    }
}
```

Flags de CLI que importan en CI: `forge test --fuzz-runs 256 --fuzz-seed 0x1234` (reproducibilidad
exacta del run que falló) y `forge coverage --report lcov` (solo orientativo — ver 19.8).

### 19.2.2 Invariant testing con handler contracts

Sin handler, Foundry llama funciones al azar del contrato objetivo y rompe precondiciones
(`onlyRole`, rutas ya ejecutadas) produciendo reverts ruidosos sin señal. El patrón correcto:

1. **Handler** = contrato puente con funciones de alto nivel que garantizan precondiciones
   (genera `routeHash` únicos, fondea al caller, respeta el pause) y envuelve cada llamada.
2. **Ghost variables** = estado espejo que el handler mantiene en el lado del test (contadores,
   sumas). La aserción del invariante compara contrato vs espejo, no el contrato contra sí mismo.
3. **Acutamiento**: `targetContract(address(handler))` excluye al contrato bajo test de llamadas
   directas; `targetSelector(FourByte(sel), address(handler))` reduce aún más el espacio;
   `targetSender(addr)` acota los actores (los callers que el fuzzer puede usar).
   `vm.excludeContract` / `vm.excludeSelector` para sacar utilería del fuzzer.
4. **Política de reverts**: con `fail-on-revert = false`, el handler hace `try/catch` y solo
   propaga (revierte) si la razón NO está en la allowlist (`Contract paused`, `Route already
   executed`, `Profit below threshold` del núcleo §2.1). Un revert desconocido = fallo de test.
   Alternativa: `fail-on-revert = true` con handlers que estructuralmente nunca revierten.

```solidity
import {StdInvariant} from "forge-std/StdInvariant.sol";
import {Test} from "forge-std/Test.sol";

contract ExecutorHandler is Test {
    ArbitrageExecutor public immutable executor;
    uint256 public ghostExecuted;            // espejo del mundo esperado

    constructor(ArbitrageExecutor _executor) { executor = _executor; }

    function executeFreshRoute(uint256 seed) external {
        bytes32 routeHash = keccak256(abi.encode(seed, block.timestamp));
        try executor.executeArbitrage(routeData(seed), amountIn(seed), routeHash) {
            ghostExecuted++;
        } catch Error(string memory reason) {
            require(allowedReason(reason), unicode"unexpected revert"); // falla el invariant
        }
    }
}

contract ExecutorInvariants is StdInvariant, Test {
    ExecutorHandler handler;
    ArbitrageExecutor executor;

    function setUp() public {
        executor = new ArbitrageExecutor();
        handler = new ExecutorHandler(executor);
        targetContract(address(handler));     // el fuzzer SOLO habla con el handler
    }

    function invariant_routes_execute_once() public view {
        assertEq(handler.ghostExecuted(), executor.executedRouteCount());
    }
}
```

Si el contrato no expone un contador (`executedRouteCount` arriba), agrégalo como `view` trivial o
reconstrúyelo contando events — nunca leas storage privado ni trackees el espejo con la misma
fuente que alimenta al contrato (tautología, ver 19.8).

Invariantes mínimos del `ArbitrageExecutor` (núcleo §2.1): (a) una `routeHash` jamás se ejecuta dos
veces; (b) `paused == true` bloquea toda ejecución sin excepción; (c) el balance del contrato
nunca decrece por una ejecución completa salvo `emergencyWithdraw` con `ADMIN_ROLE`; (d) todo
`executeArbitrage` exitoso emite `ArbitrageExecuted` con `netProfit >= minProfitThreshold`.

### 19.2.3 Fork tests pineados: por qué pinear elimina flakiness

Un fork sin pin bifurca en `latest`: el estado (reservas, precios, base fee) cambia entre corridas
y todo `assertGt` sobre valores exactos se vuelve una moneda al aire. Pinear congela dos cosas:
el bloque (estado) y el comportamiento dependiente de `block.number`/`block.timestamp`
(`vm.roll`/`vm.warp` lo controlan dentro del test).

```solidity
uint256 forkId;

function setUp() public {
    // vm.rpcUrl lee el alias de [rpc_endpoints] en foundry.toml — nada de URLs hardcodeadas
    forkId = vm.createSelectFork(vm.rpcUrl("mainnet_archive"), 19_000_000);
}

function test_plan_still_profitable_after_hostile_tx() public {
    vm.transact(0xABC...); // re-ejecuta una tx REAL del bloque sobre el estado actual del fork
    (uint256 out,,,) = quoterV2.quoteExactInput(path, amountIn);
    assertGt(out, minOut);
}
```

Cheatcodes de fork que sí existen y usarás: `vm.createFork(urlOrAlias, blockNumber)` crea sin
seleccionar; `vm.selectFork(forkId)` cambia el fork activo; `vm.rollFork(blockNumber)` avanza el
fork activo a un bloque exacto; `vm.rollFork(forkId, blockNumber)` el indicado; `vm.activeFork()`
para aserciones de saneamiento; `vm.makePersistent(addr)` para que un contrato sobreviva al cambio
de fork. `vm.transact(txHash)` reinyecta una transacción histórica real — es la herramienta base
del testing defensivo de 19.6. Condiciones de viabilidad: RPC **archive** (los nodos pruned solo
sirven ~128 bloques atrás) y bloques pineados con ≥7 días de antigüedad para que ningún provider
los podará jamás.

### 19.2.4 Cheatcodes esenciales (tabla de uso canónico)

| Cheatcode | Uso en un motor de arbitraje |
|---|---|
| `vm.prank(addr)` / `vm.startPrank(addr)` (y overload con `tx.origin`) | ejecutar como `EXECUTOR_ROLE` vs atacante anónimo |
| `vm.warp(ts)` | vencer/validar `deadline`; ventanas TWAP del oráculo (núcleo §8.2) |
| `vm.roll(block)` | lógica dependiente de `block.number` (target de bundle en `N+1`) |
| `vm.deal(addr, eth)` / `deal(token, to, amt)` (StdCheats) | fondear gas del executor; setear balances ERC20 sin transfer |
| `vm.expectRevert(bytes("Route already executed"))` | asertar la razón EXACTA, no solo "revirtió" |
| `vm.mockCall(target, calldata, returndata)` | SOLO para bordes externos (RPC, router de terceros) — jamás para el módulo bajo test (19.8) |
| `vm.expectCall(target, calldata)` | el bundle toca exactamente las pools planificadas (anti ruta-fantasma) |
| `vm.label(addr, "univ3_pool_30bps")` | traces legibles cuando el test revienta |
| `vm.pauseGasMetering()` / `vm.resumeGasMetering()` | excluir el setup del gas report |

### 19.2.5 Gas: `forge snapshot`, gas reports y gas como aserción

`forge test --gas-report` da la tabla por función; `forge snapshot` escribe `.gas-snapshot` y
`forge snapshot --diff` compara contra el archivo — versiona el snapshot y haz que un delta >5%
en hot-path haga fallar el CI. Para fuzz, la config `max-anchor-time` acota cuánto tiempo Foundry
estabiliza ("ancla") la medición de gas antes de aceptarla: con presupuesto de CI estricto,
redúcela, no la desactives. El cierre del razonamiento: como `net_profit` resta gas (núcleo
§1.2), un delta de gas es un delta de PnL aunque la lógica siga "correcta". El techo pertenece al
test, no al ojo humano: mide el gas del ciclo completo con `vm.lastCallGas()` y
`assertLt(gasUsed, GAS_CEILING)` — y congela el setup con `vm.pauseGasMetering`.

## 19.3 Differential testing contra la referencia canónica

La matemática portada al motor (CPMM, concentrated liquidity, Curve) se valida contra la
implementación del PROTOCOLO, no contra sí misma. Fuentes canónicas reales: `QuoterV2`
(`quoteExactInput`, mainnet `0x61fFE014bA17989E743c5F6cB21bF9697530B21e`) vía `eth_call` con
`--block` pineado; `UniswapV2Library.getAmountsOut` (librería on-chain del periphery); los SDK
oficiales en TS (`@uniswap/v3-sdk`: `SwapMath`, `TickMath`); Balancer `Vault.queryBatchSwap`
(consulta real vía `eth_call` — es `payable` pero query-only); Curve `get_dy`. Cada fuente
cubre un miembro de la matriz de DEX del motor.

Flujo del patrón **generated-table+probe** (patrón establecido del repo):

1. **Generador** (script fail-fast, corre rara vez, nunca en CI): para N vectores
   {pool, path, amountIn, block pineado} consulta la referencia CANÓNICA y escribe la tabla
   versionada (JSON) **in-crate**. Si un vector no se puede generar, aborta — nada de filas a medias.
2. **Probe** (corre en cada build, offline): carga la tabla, ejecuta la implementación local sobre
   cada vector, compara con tolerancia EXACTA — 0 para enteros idénticos; si el redondeo propio
   difiere, se documenta el delta permitido y por qué, no se "afloja" la tolerancia.
3. **Regeneración** = PR aparte cuyo diff de vectores se revisa como código: nuevo rango, nuevo
   pool, nueva fee tier — nada se regenera en silencio.

Regla anti-circularidad (la más violada de esta biblioteca): los vectores provienen SIEMPRE de la
referencia externa. Si se regeneran con la implementación local, el probe queda tautológico. El
precedente del repo lo paga caro: una certificación con cientos de miles de simulaciones y cero
passes demostró que un harness y un motor que comparten defecto se validan mutuamente sin decir
nada del mundo real — la fuente externa es lo único que rompe esa simetría.

## 19.4 Property testing en Rust: proptest, criterion, miri

### 19.4.1 Strategies para U256 y decimales

Generar enteros anchos con densidad en las zonas que rompen motores: cerca de 0, cerca de
`u128::MAX`, cerca del techo de reservas. `proptest` persiste los casos fallidos en
`proptest-regressions` y los re-ejecuta primero en cada corrida — ese archivo se commitea.

```rust
use proptest::prelude::*;
use alloy_primitives::U256;

prop_compose! {
    fn arb_amount_in()(bytes in prop::collection::vec(any::<u8>(), 32)) -> U256 {
        U256::from_be_slice(&bytes)          // uniforme en todo el rango U256
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]  // o PROPTEST_CASES=2048 en env

    #[test]
    fn quote_monotonic_in_amount(a in arb_amount_in()) {
        let q1 = quote(a);
        let q2 = quote(a.saturating_add(U256::from(1)));
        prop_assert!(q2 >= q1, "monotonicity broken at {:?}", a);
    }
}
```

Para sesgar hacia los bordes usa `prop_oneof!(1 => Just(U256::ZERO), 4 => rango_bajo, 1 => techo)`;
para decimales, genera el par (valor, exponente 0..18) y compone — nunca generes floats para
montos on-chain. Propiedades canónicas del dominio:

- **Monotonía output-vs-input**: `quote(x) <= quote(x + 1)` para todo swap sano, en cada hop.
- **Conservación de invariantes**: en CPMM, `x * y` post-swap >= `k` pre-swap (el redondeo a
  favor del pool se documenta); la suma (executor + fees + flash loan devuelto) cuadra el delta.
- **Idempotencia**: la misma detección produce el mismo plan determinístico y la misma
  `idempotency_key` (núcleo §1.3); re-simular no muta el plan.
- **Round-trip**: `swap(swap(a)) <= a` — si devuelve >, o el pool está regalando dinero (bug de
  la fórmula local) o el motor encontró un bug de otro y hay que frenar y auditar.
- **No panic / fail-honest (R8)**: ninguna entrada produce `unwrap` — las degeneradas devuelven
  `Err(razón_exacta)`; `None` = no computado, jamás un cero inventado.

### 19.4.2 criterion para hot paths y miri para UB

`criterion` sobre los hot paths del motor (scoring de rutas, ciclo de sim, update del grafo de
pools): `bench_function` + `std::hint::black_box` sobre las entradas; `--save-baseline` /
`--baseline` para comparar contra la referencia del PR — una regresión de latencia del detector
es una pérdida de opportunities tan real como un bug de lógica. `cargo +nightly miri test` donde
haya parsing/encoding manual (ABI decode de `routeData`, streaming del mempool): detecta UB y
out-of-bounds que pasan años sin explotar; no cubre FFI ni sockets — marca esos tests con
`#[cfg_attr(miri, ignore)]`.

## 19.5 Fuzzing de contratos: echidna/medusa (discovery) + slither (gate estático)

Foundry cubre el fuzzing dentro del test suite; echidna y medusa aportan campañas largas con
**corpus persistente** — las secuencias que rompieron algo quedan sembradas y se reintentan en
cada corrida para siempre. Config mínima real de `echidna.yaml`:

```yaml
testMode: assertion          # assert/revert = fallo; alternativas: property, optimization
testLimit: 500000
seqLen: 50
corpusDir: "corpus/echidna"  # persistir y commitear el corpus
sender: ["0x1000000000000000000000000000000000000001"]
filterWhitelist: true
filterFunctions:
  - "executeArbitrage(bytes,uint256,bytes32)"
  - "emergencyPause()"
```

En modo `property`, las invariantes son funciones `echidna_<nombre>() returns (bool)` (true =
sano); en modo `assertion`, cualquier `assert`/revert inesperado falla — reusa los invariantes de
19.2.2. Medusa agrega paralelismo (workers) sobre la misma idea; su `medusa.json` mínimo usa las
claves `fuzzing.workers`, `fuzzing.testLimit`, `fuzzing.callSequenceLength`,
`fuzzing.targetContracts`, `fuzzing.testPrefixes: ["property_"]` y `fuzzing.corpusDirectory`,
generado con `medusa init`. Ambos corren NIGHTLY (19.7), no en el camino del merge.

`slither` corre ANTES, como gate estático barato del PR: `slither contracts/ --fail-low
--exclude-dependencies` (exit != 0 bloquea). El triage inicial produce una allowlist de
supresiones documentadas con `// slither-disable-next-line <detector>` — cada línea suprimida
lleva justificación y dueño; `--fail-pedantic` queda para auditorías puntuales, no para CI.

## 19.6 Harness de simulación/replay: divergencia sim-vs-chain y golden blocks

El núcleo §9.2 esboza el replay; aquí está el contrato de calidad del harness:

1. **Pin**: `(block_number, block_hash)` del bloque histórico + RPC archive.
2. **Estado**: `CacheDB` de revm hidratado contra el fork pineado (el mismo patrón del simulador
   del núcleo §1.2), o el fork de anvil directamente en el test de Foundry.
3. **Reproducción**: re-ejecutar las txs del bloque hasta la posición objetivo (`vm.transact` en
   forge; `TxEnv` sobre `BlockEnv` pineado en revm) para llegar al estado exacto que vio el motor.
4. **Ejecución**: correr detección + simulación + gates sobre ese estado, sin red ni reloj real.
5. **Comparación**: resultado del sistema vs outcome real registrado on-chain.

**Divergencia sim-vs-chain** como métrica de salud continua: p95 de `|profit_sim - profit_real|`,
% de aciertos de signo, % de decisiones correctas (detect vs ignore) sobre la muestra diaria. La
tendencia importa más que el valor absoluto: divergencia creciente = el mundo cambió (fee switch
activado, router migrado, gas oracle distinto) y el modelo quedó atrás — alerta antes de que el
paper ledger "parezca" sano.

**Suite de golden blocks** = replay curado de N bloques etiquetados a mano:

| Etiqueta | El bloque contiene | El sistema debe |
|---|---|---|
| `should_have_detected` | arb real incluido dentro de nuestros universe/gates | detectar, simular pass, producir plan |
| `should_have_stayed_silent` | señuelo: pool manipulada, honeypot, sandwich de otro actor, token fuera de allowlist | NO emitir opportunity (o rechazar con razón exacta R8) |

Una suite con solo `should_have_detected` deja pasar a un motor sobre-ansioso (detecta hasta las
trampas y aun así pasa); una suite con solo `should_have_stayed_silent` deja pasar a un motor
ciego (no detecta nada y aun así pasa). Cada fallo de la suite registra la razón exacta y queda PROHIBIDO
re-etiquetar el bloque para que pase (precedente repo: el mislabel `viable=0` enseñó que
re-etiquetar convierte un bug en doctrina). El testing defensivo vive aquí: reproducir en el fork
el bloque donde un sandwicher ejecutó contra la pool objetivo y verificar que los gates del
sistema YA NO aprueban el plan (la opportunity fue consumida por el adversario). Enmarque
obligatorio: estas secuencias hostiles se reproducen para verificar que nuestras defensas cierran
— jamás como recetas operativas contra terceros (ver skill `arbx-mev-ethics-gate`).

## 19.7 CI gating: qué bloquea merge, qué corre nightly, anti-flaky

| Job | Contenido | Disparo | Presupuesto |
|---|---|---|---|
| `merge-gate-fast` | `cargo test --workspace`, clippy/fmt, `forge test` (unit + fuzz `runs=256`, seed fija), probes golden (19.3), `slither --fail-low`, `forge snapshot --diff` | cada PR | ≤ 10 min |
| `merge-gate-fork` | fork pineado core: 10-30 tests de rutas/sim sobre bloques con ≥7 días de antigüedad | cada PR | ≤ 20 min |
| `nightly-deep` | fuzz `runs=50000` con seed nueva (cada fallo se congela como caso explícito), invariantes extendidos (`runs=1024, depth=1000`), echidna + medusa (2 h c/u con corpus), replay de 24 h de bloques, regeneración differential + revisión de diff | cron | ≤ 8 h |
| canary | Nada de esto es canary: L6 es acción de operador, gated CLAUDE.md §34 | operador | n/a |

Reglas anti-flaky (innegociables): (1) semilla fija en todo lo que bloquea merge — un fallo debe
poder reproducirse con el mismo comando; (2) reloj fakeado: ninguna capa bajo L5 lee
`Date.now`/`SystemTime` directo; el clock se inyecta y se warpea (mismo espíritu que R1 en
frontend); (3) sin red real bajo L4 — unit/property no abren sockets ni RPC; los fallos de RPC
del fork son fallos de infra, se les trata con reintentos del JOB, nunca del test; (4) un test
flaky se marca flaky, se investiga y se arregla — rerun-until-green es la forma más cara de no
saber nada; (5) presupuesto explícito: si un job no cabe en su presupuesto, se recorta el scope
del job, no el determinismo.

## 19.8 Antipatrones (y su contramedida)

| Antipatrón | Por qué mata | Contramedida |
|---|---|---|
| `vm.mockCall` de la función bajo test | el test valida el mock, pasa siempre | mocks solo en bordes externos (RPC, routers de terceros); el módulo bajo test corre real |
| Snapshots/forks sin determinismo (tip sin pin, `Date.now`, `Math.random`) | verde intermitente = verde inventado | pin de bloque + seed fija + clock inyectado |
| Vectores golden regenerados con la implementación local | probe tautológico, cero señal | el generador solo habla con la referencia externa; regen = PR aparte |
| Coverage vanity (90% líneas, 0 invariantes) | las líneas no predicen bugs de propiedad | medir: propiedades enumeradas CON test, invariantes con aserción viva, % de golden blocks verdes |
| Happy path solamente | los bordes son donde vive el bug | fuzz de degenerados: 0, 1 wei, `u128::MAX`, reserva 1 wei, fee máxima |
| Tolerancia holgada "para que pase" | esconde redondeos incorrectos que comen PnL | tolerancia 0 para enteros; el delta de redondeo se documenta, no se esconde |
| Re-etiquetar golden blocks para quedar verde | convierte el bug en doctrina | prohibido; el fallo queda como evidencia (R8: razón exacta) |
| Ignorar un flaky con retries | CI que llora lobo | flaky = fail del job hasta root-cause |

Cierre del razonamiento de toda la referencia: la cobertura de LÍNEAS se celebra; la cobertura de
INVARIANTES es la que evita el incidente. Un motor de arbitraje no falla en la línea 47 — falla
en el caso `amountIn = 2^128 - 1` con reserva de 3 wei en el hop 4 de una ruta de 5, a las 3 AM,
con capital dentro. La pirámide existe para que ese caso muera en L2, gratis, en 40 ms.

## 19.9 Testing del control-plane TS/Edge

La pirámide 19.1 vive en Rust/Foundry; el control-plane (frontend Next.js 14, edge worker Hono,
shared-ts) no tiene bytecode que fuzze ni fork que pinear — su "mundo real" es el payload del wire
y los eventos del bus. Esta sección es la pirámide equivalente, y el repo ya tiene el patrón
canónico asentado: `frontend/lib/__tests__/api-json-vs-zod-contract.test.ts` y los specs
`frontend/e2e/*.spec.ts`. Regla puente con 19.1: ningún bug atrapable en unit llega a E2E — el
E2E es la capa más cara por corrida, no el lugar de descubrir un bug de redondeo de formato.

### 19.9.1 Contratos de API: fixtures grabados contra el schema zod

Un endpoint es un contrato; el test de contrato congela ese contrato del lado real (la respuesta
que la API sirvió) y lo parsea con el MISMO schema zod que el cliente valida en runtime
(`api-client.ts`, `getValidated`): el schema canónico vive en `frontend/lib/schemas.ts` y
`frontend/lib/operations-schemas.ts`, con espejo server-side en `shared-ts/src/api-contracts.ts`;
el test importa ese módulo, jamás una copia para testear.

1. **Grabar** (rara vez, manual, read-only): respuestas REALES de la API desplegada a un directorio
   de fixtures versionado, con un manifest que registre URL, método, bytes y sha256 por archivo —
   integridad verificada ANTES de parsear (así opera el test canónico con `prod_20260824`).
2. **Parsear** (en cada CI, offline): `schema.safeParse(JSON.parse(fixture))` — el fixture entra
   como `unknown`; el trabajo de tipar es del schema, no de un cast.
3. **Regenerar** = PR aparte cuyo diff se revisa como código (espejo de 19.3): nunca en silencio.

```ts
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { KpiPayloadSchema } from "@/lib/operations-schemas";

const FIXTURES = join(dirname(fileURLToPath(import.meta.url)), "fixtures", "prod_20260824");

describe("contract: wire vs KpiPayloadSchema", () => {
  it("the live schema accepts the recorded production payload", () => {
    const wire: unknown = JSON.parse(readFileSync(join(FIXTURES, "operations_kpi.json"), "utf8"));
    const parsed = KpiPayloadSchema.safeParse(wire);
    expect(parsed.success).toBe(true); // failure = wire<->schema drift: a finding, never loosen
  });
});
```

La dirección del fallo es innegociable (RULE 00): la API cambia sin actualizar el schema → el test
ROMPE → el PR arregla schema Y graba el fixture nuevo de la respuesta real. Lo prohibido es la
dirección inversa: editar el schema (o el fixture) para hacer pasar el test sin reproducir la
respuesta real — es exactamente la anti-circularidad de 19.3: si el "fixture" se deriva del
schema, el contrato queda tautológico. Honradez de alcance (R8): los endpoints sin schema
(consumidos con fetch crudo) se registran como finding documentado — el gap es el hallazgo, no se
fabrica el fixture.

### 19.9.2 Fixtures de replay WS y golden de render

El dashboard es un consumidor de eventos del bus (Redis Streams vía api-server → socket.io). Su
test determinista graba una **secuencia** y la reproduce:

1. **Grabar**: `XRANGE arbx:opps:detected - +` (rango ordenado del stream, R7) o los eventos que
   api-server emitió por socket.io, con sus timestamps relativos → fixture JSON ordenado
   `{ at_ms, event, payload }[]`. Es el golden block de 19.6, del lado del bus.
2. **Reproducir**: el seam de mock es SOLO el borde de transporte (el wrapper de socket.io-client);
   store, hooks y componentes corren reales — mismo criterio que `vm.mockCall` en 19.2.4. El reloj
   se fakea y AVANZA según la grabación: `vi.useFakeTimers()`, `vi.setSystemTime(new Date(...))`,
   `vi.advanceTimersByTimeAsync(0)` para flush determinista de microtasks.
3. **Golden de render**: tras el replay, el estado y el markup se comparan contra golden
   (`toMatchFileSnapshot` en vitest 3, o `renderToStaticMarkup` de `react-dom/server` + snapshot
   del string). Datos fijos, semilla fija; si se necesita azar, PRNG seeded — cero `Date.now` y
   cero `Math.random` sin semilla en test o en componente bajo test. Las fuentes de flakiness del
   lado TS son contables — reloj real, azar sin semilla, orden de resolución de
   promesas/interleaving de timers, red/WS real fuera de E2E (19.8, 19.9.5) — y cada una tiene su
   contramedida enumerada; no hay ranking místico. En componentes, R1 ya confina lo no
   determinístico al `useEffect` (la anti-reincidencia nació de esa clase de bug de hidratación).

### 19.9.3 Capas TS: unit → contrato → componentes → E2E

| Capa | Herramienta (repo) | Atrapa | No puede atrapar |
|---|---|---|---|
| unit | vitest sobre funciones puras (`frontend/lib/__tests__/format.test.ts`) | formato, scoring, edge de parseo local | lo que dicta el wire |
| contrato | zod `safeParse` vs fixtures grabados (19.9.1) | drift wire↔schema, campo que la API dejó de mandar | comportamiento del consumidor ante el evento |
| componentes | render con datos fijos + golden (19.9.2) | estado vacío R8 mal renderizado, hidratación, orden de columnas | wiring real de red y auth |
| E2E | `@playwright/test`, specs reales (`opportunities-honest-display`, `mirror_fidelity`, `readiness-smoke`, `paper-mode-alignment`) | journeys de usuario, estados vacíos honestos R8 EN PANTALLA | por qué falló (caro, lento, una foto) |

Mapeo explícito con 19.1 (extensión, no reinvención): contrato ≈ L2/L3 — el schema zod ES el
invariante del wire; componentes ≈ L4 de integración local (render+store, sin red); E2E ≈ L4/L5
del lado usuario (despliegue real, read-only, sin broadcast). La regla (2) de 19.1 corre igual
aquí: cada incidente del dashboard aterriza como test en la capa MÁS BAJA que lo reproduzca — un
drift wire↔schema se descubre en contrato, jamás en E2E. Precisión de alcance (R8): ningún spec
e2e referencia el bus ni socket — el contrato por evento vive en las capas bajas
(`websocket-client.test.ts` con socket.io-client mockeado, 19.9.2); el E2E solo ejercita el WS
real de fondo.

El E2E de honestidad es obligatorio y ya existe: abrir la página sin datos y exigir el estado
vacío con su razón exacta — prohibido que el journey "pase" con un spinner eterno o un cero
inventado (R8 en pantalla). Corresponde con núcleo §10 igual que el resto del capítulo.

### 19.9.4 Gates de CI del control-plane

Bloqueantes en cada PR, con los scripts que ya existen en `package.json` de cada paquete TS
(`frontend`, `edge/worker`, `backend/api-server`, `shared-ts`): `typecheck` (`tsc --noEmit`) y
`test` (`vitest run`; en `edge/worker` hoy es `vitest run --passWithNoTests`) existen en los
cuatro; `lint` con ESLint real existe SOLO en `frontend` — `api-server` y `shared-ts` llevan
placeholders `echo` y `edge/worker` ni siquiera tiene el script: ese gap es un finding R8
documentado, no un verde de tabla — y Playwright (`npx playwright test`, config
`playwright.config.ts`) donde su presupuesto esté definido (espejo de 19.7: presupuesto
explícito; nunca "verde a veces"). Tres reglas duras:

1. **Lockfile pineado + `npm ci`** (nunca `npm install` en CI): sin lockfile, cada corrida resuelve
   un árbol de dependencias distinto → CI no-reproducible = CI roto aunque esté verde. Precedente
   repo: el frontend sigue sin lockfile propio (verificado 2026-09-15; solo existe el lockfile
   raíz) y el typecheck dependía del hoisting — eso no es un entorno de test, es una lotería.
2. **El guard también es un test, y debe correr**: `next.config.guard.test.ts` (R2) estuvo
   silenciosamente SIN colectarse por un patrón de include que no lo cubría — un guard que nunca
   corre es peor que ninguno (EDGE-HARD-1). Verifica que aparece en el output de `vitest run`.
3. **El retry de E2E es triage, no absolución**: `playwright.config.ts` corre en CI con
   `retries: 2` — un test que solo pasa en reintento es un flaky confeso (regla 4 de 19.7): se
   root-causa, no se celebra el verde.

### 19.9.5 Antipatrones TS (y su contramedida)

| Antipatrón | Por qué mata | Contramedida |
|---|---|---|
| Mock del contrato editado junto con la API (acoplamiento nulo) | el test valida el mock, pasa siempre | fixture grabado con manifest+sha256; regen solo desde la API real, PR aparte (19.9.1) |
| Editar el schema zod para acomodar el fixture | convierte el drift en doctrina (RULE 00) | el schema solo cambia junto con un fixture NUEVO grabado de la respuesta real |
| Snapshot sin semilla/datos fijos (`Date.now`, `Math.random` sin semilla) | verde intermitente = verde inventado | `vi.useFakeTimers` + `vi.setSystemTime` + PRNG seeded; datos del fixture, nunca generados |
| Tests que dependen del reloj real o del orden de resolución de promesas | carrera = flaky = test muerto (19.7) | reloj inyectado; aserción sobre estado final tras flush explícito (`advanceTimersByTimeAsync`), no sobre orden intermedio |
| Coverage-vanity (90% líneas, 0 contratos verdes) | las líneas no predicen drift | medir: endpoints con fixture+schema verdes, goldens de replay verdes, journeys E2E verdes |

El cierre de 19.8 aplica igual aquí: la cobertura de líneas se celebra; la de contratos y goldens
es la que evita el incidente a las 3 AM cuando api-server cambia un campo y el dashboard rompe en
producción con `undefined` en pantalla.

## GOBERNANZA

Todo el conocimiento aquí está subordinado a los gates `arbx-*` (arbx-paper-trade-first,
arbx-simulation-mandatory, arbx-risk-limits-enforcement, arbx-pre-execute-checklist) y a
CLAUDE.md §34 (LIVE_MAINNET gated, default-deny en `relays-client`). L6/canary es acción de
operador bajo §34.5, no un paso de CI. Nada de esta referencia autoriza flip a live ni broadcast
con capital real; el fuzzing de secuencias hostiles es defensivo y se rige por
`arbx-mev-ethics-gate`.
