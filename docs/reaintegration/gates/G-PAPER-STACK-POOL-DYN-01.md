# G-PAPER-STACK-01 (Pool dinámico) — el Pool de Aave V3 se resuelve con `getPool()` (POOL-DYN-01)

- **Tipo:** feature (dejar de exigir `AAVE_POOL_<chain_id>`) + gate que ejecuta el camino REAL.
- **Script:** `scripts/gates/g-paper-stack-01.sh` (exit 0 = PASS, 1 = FAIL, 2 = SKIP sin RPC/toolchain).
- **Implementación:** `backend/shared-rs/src/chains.rs` (catálogo + selector),
  `backend/simulator-v2/src/sequence_runner.rs` (`read_call`),
  `backend/sim-core/src/paper_executors.rs` (`PaperStackSpec` + `resolve_aave_pool_from_provider`).
- **Base:** `origin/main` = `21b093b7` (rama local `rea-pool-dyn-01`).

## El ítem

`AAVE_POOL_<chain_id>` era **obligatorio** (`shared-rs/src/chains.rs::resolve_aave_pool_address`,
PR-D1): sin la variable, `for_round_trip` devolvía `Err` y el paper stack no desplegaba. El Pool
de Aave V3 **no es una constante**: es el ancla de governance la que se fija por chain, y el Pool
se resuelve **on-chain** con `IPoolAddressesProvider.getPool()` — doctrina RULE 00
(`.claude/skills/arbx-live-engineering/references/biblioteca/14-onchain-execution-contracts.md`:
*"resolver `getPool()` en vez de hardcodear"*). Así una governance `setPool` se recoge sin tocar código.

## Diseño

- **Catálogo de providers** (`chains.rs`): `AAVE_V3_POOL_ADDRESSES_PROVIDER` (const, `(chain_id, provider)`)
  para `1, 42161, 10, 137, 43114, 8453, 56, 11155111, 421614, 11155420`, más el selector
  `AAVE_V3_GET_POOL_SELECTOR = 0x026b1d5f` (`cast sig "getPool()"`). **Solo el provider**, nunca un Pool.
- **`read_call`** (`sequence_runner.rs`): primitiva de lectura genérica, **no-committing** (ni gas, ni
  `successful_calls`, ni nonce, ni traza), `None` si el destino no tiene código, error tipado si revierte.
- **`PaperStackSpec`**: `aave_pool: Option<Address>` + `chain_id: u64`. `for_round_trip` rellena
  `Some(pool)` **solo** si `AAVE_POOL_<chain_id>` está puesto (override explícito; sigue ganando, y un
  valor mal formado o cero sigue rechazando — nunca se descarta en silencio); si no, `None`.
  El `RoundTripContext` **no lleva provider**, por eso `for_round_trip` no puede leer on-chain: el
  `SequenceContext` solo existe en `deploy_paper_executor_stack`.
- **`resolve_aave_pool_from_provider`** (`paper_executors.rs`): con `None`, `deploy_paper_executor_stack`
  busca el provider del `chain_id` en el catálogo y lee `getPool()`. **Falla cerrado** en cada camino
  adverso: sin provider en el catálogo, lectura fallida, dato vacío, retorno ≠ 32 bytes, o `0x0`.
  **Nunca un Pool inventado.**
- El **env queda como override** — el comportamiento de hoy (cuando el env está) es idéntico.

## Gate (6 pasos)

1. `paper_stack_wired_path_deploys_ok_at_call_site_gas` — camino CABLEADO con el gas real de cada call site (PR-D2).
2. `paper_stack_uncapped_gas_is_rejected_by_the_osaka_tx_cap` — causa raíz EIP-7825 (PR-D2).
3. `paper_stack_deploy_initializes_via_erc1967_proxy_ok` — deploy vía proxy (PR-D1).
4. `paper_stack_pool_resolution_fails_closed*` — (c) offline: sin provider / lectura fallida ⇒ `Err` tipado.
5. `paper_stack_pool_env_override_beats_provider` — (a) el override gana (leído de vuelta del FLE desplegado).
6. `paper_stack_pool_resolved_from_provider_matches_mainnet_canonical` — (b)+(d) `getPool()` en un
   **fork de mainnet** == `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` (paridad mainnet).

Los pasos 5-6 leen `getPool()`: el `PAPER_STACK_RPC` debe ser un **fork de mainnet** (un `anvil` pelado
no tiene provider y el paso 6 se pone rojo, que es lo correcto).

## Resultado (medido, 2026-10-18)

```
paso 5: PAPER_STACK_POOL_ENV_OVERRIDE_WINS=0x1111…  (override gana a getPool())
paso 6: PAPER_STACK_POOL_FROM_PROVIDER=0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2
G-PAPER-STACK-01 — PASS
```
