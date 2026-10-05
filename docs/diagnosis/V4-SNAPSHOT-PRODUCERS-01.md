# V4-SNAPSHOT-PRODUCERS-01 — capital_usd y costs.{execution_fees,financing}: atribución medida

Tarea: cerrar el gate que bloquea **64/269** cartuchos con
`applicable_data_or_constraint_gap` (medido por ARSE-264-01 en producción,
2026-10-03). Todo lo de este documento está medido con comando; el VPS se leyó
**solo-lectura** y no se tocó nada (capital expuesto = 0).

## 1. Atribución: quién produce cada campo y por qué falta

| Campo | Productor (archivo:línea, `origin/main` 697dc472) | Consumidor que lo exige | Razón MEDIDA de la ausencia |
|---|---|---|---|
| `capital_usd` | `snapshot_services.rs::quote()` — `capital = self.price(&first.token_in)` valorado con `token_value_usd`, alimentado por `SnapshotBundle.prices` construido en **`backend/searcher-rs/src/cartridge_boot.rs:2040-2099`** (`build_v4_intent_bundle`) | `rhai_agent_bridge.rs:589-601` → `capital_missing_or_cap_exceeded` | El token de la primera pierna no tiene entrada en `prices`: el mapa se llena sólo si (1) `identity.symbol_for_addr(token)` resuelve **y** (2) alguna de las TRES fuentes de precio, todas keyed por **símbolo**, tiene valor positivo. Sin precio → sin entrada → `quote.capital_usd = None`. |
| `costs.execution_fees` | línea `embedded` de `v4_base_cost_lines` (**`cartridge_boot.rs:2241-2250`**, `usd: None` por diseño) + el valor exacto que la rellena en `snapshot_services.rs:1074-1079` vía `ledger_execution_fees_usd` (`snapshot_services.rs:340-…`) | `rhai_agent_bridge.rs:515-538` → `missing_or_invalid_cost` | La línea EXISTE con `usd: None`: el relleno exacto por ledger exige el precio del `token_in` de la pierna (con fallback al token base del ciclo); sin ninguno de los dos precios, el valor no es computable y la línea queda sin USD (comportamiento honesto ya documentado en el código). |
| `costs.financing` | `v4_base_cost_lines` (**`cartridge_boot.rs:2199-2238`**) | `rhai_agent_bridge.rs:562-573` → `mandatory_route_cost_missing` | **Defecto del productor (corregido aquí)**: la línea sólo se empujaba DENTRO del `if let Some(amount_usd)`. Con `flashloan_fee_pct > 0` y sin precio del start token, la línea **desaparecía en silencio** → el bridge reportaba "no hay productor" cuando el productor existe y lo que falta es el precio. |

**No es "el contrato v4 no los declara" ni "el consumidor los busca con otro
nombre":** el contrato está (`QuotedPlan.costs` / `required_cost_kinds`,
`rhai_agent_bridge.rs:81-115`), el consumidor los busca con el nombre correcto, y
la única causa común de los tres es **un precio canónico ausente para los tokens
de la ruta**.

### La cadena de precios, medida

Comandos (VPS, solo lectura):

```
docker exec arbitragex-v2-redis-1 redis-cli HLEN   arbx:token_prices:1   → 447      # hash field=SYMBOL, value=USD, TTL 55 s
docker exec arbitragex-v2-redis-1 redis-cli --scan --pattern 'arbx:token*' | wc -l → 2747   # universo de identidad
docker exec arbitragex-v2-redis-1 redis-cli GET arbx:tokens:1:0x4c11…c1b5
  → {"symbol":"ORAI","decimals":18,"is_stablecoin":false}          # SIN precio: no hay fuente address-keyed
```

`trading_config` en PG (chain 1, `SELECT … ORDER BY updated_at DESC`):
`capital_usd = 1000.00`, `base_token_symbol = WETH`,
`base_token_price_usd = 2693.5435`, `gas_estimate_units = 250000`,
**`flashloan_fee_pct = 0.0009` (> 0)**, `token_prices_usd` = **21 símbolos**.

Es decir: 447 símbolos publicados + 21 del operador frente a 2747 tokens de
identidad y un universo ruteado (mempool) arbitrariamente long-tail → la ruta
típica llega SIN precio y las tres reparaciones caen juntas, 64/64.

### Firma medida en producción (la que hay que mover)

```
docker logs arbitragex-v2-searcher-rs-1 | grep active_eval_summary | tail -1
  pertinent=269 negative=269 positive=0
  reasons: applicable_data_or_constraint_gap 64 · dispatch_needs_route_data 174 · …
  top_repairs: [("capital_usd::capital_missing_or_cap_exceeded", 64),
                ("costs.execution_fees::missing_or_invalid_cost", 64),
                ("costs.financing::mandatory_route_cost_missing", 64),
                ("operators.22::DATA_GAP", 64), ("protocol_exact_quotes::v3_within_tick_is_hypothesis_not_protocol_verified", 60), …]
```

Nota: `costs.gas` **no** aparece en `top_repairs` → la línea de gas SÍ se computa
(necesita `gas_estimate_units` + gas observado + `base_token_price_usd`, los tres
presentes). Coherente con el diagnóstico: el único eslabón roto es el precio del
token de la ruta.

## 2. Qué se corrige aquí (dentro del alcance)

`backend/searcher-rs/src/cartridge_boot.rs::v4_base_cost_lines` — la línea de
`financing` **se declara siempre**:

* con precio y tasa > 0 → `external` con la reserva computada (sin cambios);
* sin precio canónico del start token → `external`, `usd: None`, `reason` que
  nombra la causa exacta (`prices map miss` + la tasa declarada) y `evidence_id`
  `config:flashloan_fee_pct+unpriced:start_token`;
* el `treatment` sigue siendo `external` porque la financiación **APLICA**
  (tasa declarada > 0): declararla `not_applicable` afirmaría que el coste no
  existe e inflaría el neto — exactamente el "valor por defecto silencioso" que
  R8/RULE 00 prohíben.

Consecuencia en el bridge: ese caso pasa de
`costs.financing::mandatory_route_cost_missing` (razón falsa: "no hay
productor") a `costs.financing::missing_or_invalid_cost` (razón verdadera: "el
productor existe, el importe no es computable"). **No** convierte el candidato en
elegible: `net_profit_usd` sigue `null` y `candidate_eligible` sigue `false`.

Ningún valor se inventa: no hay precios nuevos, no hay ceros, no hay literales de
capital/tasa — la tasa sale de `config:flashloan_fee_pct` del operador.

## 3. Qué queda bloqueado y con qué productor (fuera de alcance, NO implementado)

Aun con los tres campos de coste resueltos, los 64 candidatos **no** serían
elegibles: `rhai_agent_bridge.rs:668-672` exige `repairs.is_empty()`. Cada
candidato arrastra además, medido:

| Reparación restante | Frecuencia | Productor que la cerraría | Alcance |
|---|---|---|---|
| `operators.22::DATA_GAP` (Monte Carlo) | 64 | `math_evidence.rs:155-209` construye el `MarketState` desde los ratios de reservas del grafo (1 fila por pierna); op_22 necesita una SERIE de precios con más filas. Productor: historial de precios por par (ventana) | `math_evidence.rs`, `searcher-rs` |
| `operators.{21,16,8,11,13,26,5,10}::DATA_GAP` | 60/32/32/27/26/23/22/15 | igual: los operadores numéricos reciben el mismo `MarketState` corto; cada uno declara su propia carencia (p.ej. op_16 Kelly sin pérdidas, op_21 Newton sin edge, op_26 flash sin break-even) | `math_evidence.rs` |
| `protocol_exact_quotes::v3_within_tick_is_hypothesis_not_protocol_verified` | 60 | productor de quote EXACTO de protocolo para las piernas V3 (exact_quotes del bundle, hoy `Default::default()`); la rama «within-tick» es hipótesis y se declara como tal | `snapshot_services.rs` / `agent_graph.rs` |
| precios canónicos de la ruta | 64 | cobertura del stack de precios: `workers/price_worker.rs` + `token-enricher` (o `trading_config.token_prices_usd` del operador) | **fuera de alcance** |

Ninguno de esos productores se tocó: son fuera del alcance declarado y este
informe los declara ANTES de tocarlos.

## 4. Lo NO verificado (con su razón)

* **`cargo` en el host de trabajo: NO compila** (`os error 4551`, AppControl
  bloquea los build scripts). El gate se acredita con la corrida de CI sobre la
  rama basada en `origin/main` (id/URL en el reporte de la tarea).
* **Bajada del contador 64 en producción: NO VERIFICABLE pre-deploy.** El fix
  cambia la RAZÓN con la que esos 64 se rechazan, no su elegibilidad (necesitan
  además los productores de §3). Estimar una bajada sería inventar el número.
* **Cobertura de precios efectiva por intent**: no se añadió un contador de
  cobertura al `active_eval_summary`; la atribución se sostiene con el código, la
  config medida y la firma de reparaciones, no con una instrumentación nueva.
