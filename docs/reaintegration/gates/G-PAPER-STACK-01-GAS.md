# G-PAPER-STACK-01 (gas) — el camino cableado debe poder encender `PAPER_STACK_DEPLOY` (PR-D2)

- **Tipo:** fix de runtime (1 línea) + gate que ejecuta el camino REAL.
- **Script:** `scripts/gates/g-paper-stack-01.sh` (exit 0 = PASS, 1 = FAIL, 2 = SKIP sin RPC/toolchain).
- **Implementación:** `backend/sim-core/src/paper_executors.rs` (`PaperStackSpec::for_round_trip` + 3 tests).
- **Base:** `origin/main` = `d3087b84` (rama local `rea-pr-d2`).

## Defecto (el ítem que impide encender el flag)

El cableado pasa `gas_limit_per_step` tal cual a `PaperStackSpec` y de ahí a **cada** transacción de deploy/setup. El call site del **scanner** pasa **`30_000_000`**:

`backend/searcher-rs/src/scanner.rs:3529` → `paper_stack_for_round_trip(&ctx, chain_id, gas_price, 30_000_000)`.

El `SequenceContext` de paper fija **`SpecId::OSAKA`** incondicionalmente (`simulator-v2/src/sequence_runner.rs:227`), y desde OSAKA **EIP-7825 capa el gas de una transacción en `2^24 = 16_777_216`**. El transactor de REVM **rechaza la tx ANTES de ejecutarla**:

```
ArbitrageExecutor deploy failed: REVM transact_commit infrastructure error:
transaction validation error: transaction gas limit (30000000) is greater than the cap (16777216)
```

⇒ Con el árbol de `main`, encender `PAPER_STACK_DEPLOY` por el scanner **no puede funcionar**: toda simulación cableada muere en el deploy del AE. Los otros call sites van por debajo del tope (16M / 16M / 500k) y la ruta verificada ya capaba (`verified_simulation.rs`: `min(gas_limit, 16_777_216)`) — la ruta **paper** se había quedado sin el cap.

## Fix (1 línea)

`PaperStackSpec::for_round_trip` capa el gas:

```rust
gas_limit: gas_limit.min(PAPER_STACK_MAX_TX_GAS)   // PAPER_STACK_MAX_TX_GAS = 16_000_000
```

Un notch por debajo del tope de consenso (`2^24`), el mismo margen que mantiene la ruta verificada. Es un **no-op** para los call sites que ya van por debajo. **Los cinco** call sites construyen el spec por este builder, así que el cap aplica a todos.

## Por qué el gate anterior estaba verde con el camino roto

`G-PAPER-STACK-01` corría **sólo** `paper_stack_deploy_initializes_via_erc1967_proxy_ok`, que construye un `PaperStackSpec` **a mano** con `gas_limit: 15_000_000` — por debajo del tope. El gas real de los call sites **no se ejercitaba** nunca. El gate ahora corre tres pasos:

1. `paper_stack_wired_path_deploys_ok_at_call_site_gas` — construye el spec por **`PaperStackSpec::for_round_trip`** (el builder de producción) con el gas real de cada call site (`PAPER_STACK_GAS_VALUES`, por defecto `30000000,16000000`) y exige deploy **`Ok`**, comprobando que el cap se aplicó (`30M → 16M`).
2. `paper_stack_uncapped_gas_is_rejected_by_the_osaka_tx_cap` — fija la **causa raíz**: un spec **sin capar** a `30_000_000` lo rechaza EIP-7825 con el mensaje exacto de arriba; si alguien retira el cap, el gate se pone rojo.
3. `paper_stack_deploy_initializes_via_erc1967_proxy_ok` — el deploy PR-D1 (proxy) sigue verificándose.

## Resultado medido (anvil, `SpecId::OSAKA`)

```
paso 1: PAPER_STACK_WIRED_AT_CALL_SITE_GAS requested=30000000 capped=16000000 OUTCOME=OK
        PAPER_STACK_WIRED_AT_CALL_SITE_GAS requested=16000000 capped=16000000 OUTCOME=OK
paso 2: PAPER_STACK_UNCAPPED_30M_REJECTED=… gas limit (30000000) is greater than the cap (16777216)
paso 3: PAPER_STACK_DEPLOY_OUTCOME=OK
G-PAPER-STACK-01 — PASS
```

Re-verificado **después** de regenerar el fixture del `FlashLoanExecutor` (PR-D2): sigue PASS.

## Declarado (no oculto): el default de sim-ctl no basta para desplegar

Los call sites de `sim-ctl` pasan su default `500_000` (`SIM_GAS_LIMIT_PER_STEP` sin definir → `sim_runner.rs:87`). Ese valor está **por debajo del suelo de deploy** del stack: medido por bisección, `2_000_000` falla `OutOfGas(Basic)` y `2_200_000` es `Ok` (el código de creación del AE solo ya deposita ~9.5 KB). Por eso `500_000` se cubre en el test offline del cap **y no** en el deploy del gate:

- El cap es un **no-op** ahí (500k < 16M) — el defecto de PR-D2 no aplica.
- Pero encender `PAPER_STACK_DEPLOY` por **sim-ctl con defaults** seguiría fallando por `OutOfGas`: hay que subir `SIM_GAS_LIMIT_PER_STEP` (> ~2.2M). Es una **limitación separada, declarada aquí**, no el defecto del cap. Decisión del propietario si quiere que el spec use otra fuente de gas para el deploy.

## Revert

Dos pasos independientes (un commit cada uno):
- revertir el commit del fix restaura el pass-through del gas;
- revertir el commit del gate elimina los dos tests `#[ignore]` y devuelve el gate a su forma anterior (sólo el deploy a 15M).
