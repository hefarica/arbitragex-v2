ID:            REA-14-SIGNED-ORDER
Fuente:        1inch/limit-order-protocol @50bd1300315606a3d32e9021898374c9d0dc2335
               contracts/extensions/AmountGetterBase.sol:17-79 (getMakingAmount/getTakingAmount + .sol:58/76 las dos mulDiv)
               contracts/OrderLib.sol, contracts/OrderMixin.sol (pipeline de fill)
Licencia:      MIT (LICENSE.md del sha: "MIT License, © 2019, 1inch") — verbatim permitido con atribución; se portaría igual
Tipo:          INSPIRACION — PASO 3 (REA_MASTER §5) NO satisfacible con vectores limpios del upstream (razones abajo)
Invariante:    makingAmount = floor(orderMakingAmount · takingAmount / orderTakingAmount);
               takingAmount = ceil(orderTakingAmount · makingAmount / orderMakingAmount)
               → el redondeo SIEMPRE favorece al maker; con `extraData.length >= 20` se delega a un getter externo.
Matematica:    AmountGetterBase.sol:58 `order.makingAmount.mulDiv(takingAmount, order.takingAmount)` (OpenZeppelin
               mulDiv, Floor por defecto)
               AmountGetterBase.sol:76 `order.takingAmount.mulDiv(makingAmount, order.makingAmount, Math.Rounding.Ceil)`
Test origen:   NO existe aserción numérica de las funciones puras: test/Eip712.js sin hashes literales
               (grep de 0x[64] vacío); test/Extensions.js solo fixtures (`makingAmount: '0'`).
               Los únicos números exactos del repo son cambios de balance de `fillOrder`
               (test/LimitOrderProtocol.js:310-312 dai ±1/weth ∓9; :347-349 dai ±4/weth ∓1; :368-370 ether ±5/±1),
               que ejercitan OrderMixin + OrderLib + permits + WETH unwrap — NO las dos mulDiv puras.
               Declarar esos balances como vectores de la fórmula sería un port falso.
Test propio:   N/A — no se escribe código en esta unidad (regla §0.3: sin test numérico del repo no hay REA).
Gate:          NO EJECUTABLE sin inventar el vector. La capacidad real que falta (ingesta + firma EIP-712 + registro
               + permit2 + accounting de fill) es un PIPELINE, no una unidad de matemática portable.
Control neg.:  N/A (sin código). El "control negativo" del estado actual: el destino NO tiene intents
               (sweep: cero matches de signed_order/limit_order/uniswapx/seaport) → la ausencia es real, pero no
               se cierra con una unidad de dos fórmulas.
Evidencia:     clones/greps ejecutados en esta iteración (rutas+línea citadas); sha y LICENSE leídos en el clone.
Riesgo:        n/a (cero cambios de código).
FOLLOWUP:      REA-14-CLOSE — proyecto de familia "intents/signed orders": (1) EIP-712 domain separator + Order
               struct hash con vectores propios; (2) ingest de órdenes firmadas; (3) registry/nonce; (4) permit2;
               (5) fill accounting sobre el ledger por pata existente. Requiere decisión del operador sobre si se
               quiere abrir esa familia (mercado nuevo, no cierre de capacidad).
Estado:        BLOCKED — INSPIRACION (PASO 3 no cumplido; la unidad tal como está en el backlog no es portable)
