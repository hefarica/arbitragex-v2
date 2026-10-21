// Vectores upstream de FICHA_REA1_ARTEMIS.md — verificacion independiente (BigInt puro, sin deps)
// Fuente: flashbots/simple-arbitrage@03cf1058b5bd8079561e2505efc85f77e357b829
//   src/Arbitrage.ts:19-29 (TEST_VOLUMES), 31-67 (getBestCrossedMarket)
//   src/UniswappyV2EthPair.ts:150-154 (getAmountIn), 156-161 (getAmountOut)
// Test del upstream: test/Arbitrage.test.ts:21-43, test/UniswappyV2EthPair.test.ts:20-27
// Ejecutar: node docs/rea/vectors_rea1_verify.mjs  (exit 0 si TODOS los vectores coinciden)
let failures = 0;
function check(name, got, expected) {
  const ok = got === expected;
  if (!ok) { failures++; console.error(`FAIL ${name}: got ${got}, expected ${expected}`); }
  else console.log(`ok   ${name} = ${got}`);
}
const ETHER = 10n ** 18n;
const WETH = "WETH", TOKEN = "TOKEN";

function getAmountIn(reserveIn, reserveOut, amountOut) {
  const numerator = reserveIn * amountOut * 1000n;
  const denominator = (reserveOut - amountOut) * 997n;
  return numerator / denominator + 1n;
}
function getAmountOut(reserveIn, reserveOut, amountIn) {
  const amountInWithFee = amountIn * 997n;
  const numerator = amountInWithFee * reserveOut;
  const denominator = reserveIn * 1000n + amountInWithFee;
  return numerator / denominator;
}
function mkPool(tokens, balances) { return { bal: Object.fromEntries(tokens.map((t, i) => [t, balances[i]])) }; }
function tokensOut(pool, tIn, tOut, amtIn) { return getAmountOut(pool.bal[tIn], pool.bal[tOut], amtIn); }

// V1 — test/UniswappyV2EthPair.test.ts:20-27 (reservas [1 TOKEN, 2 WETH])
const p = mkPool([TOKEN, WETH], [ETHER, ETHER * 2n]);
check("V1a getTokensIn(TOKEN->WETH, 0.1e18)", getAmountIn(p.bal[TOKEN], p.bal[WETH], ETHER / 10n), 52789948793749671n);
check("V1b getTokensIn(WETH->TOKEN, 0.1e18)", getAmountIn(p.bal[WETH], p.bal[TOKEN], ETHER / 10n), 222890894906943052n);
check("V1c getTokensOut(TOKEN->WETH, 52789948793749671)", tokensOut(p, TOKEN, WETH, 52789948793749671n), ETHER / 10n + 1n);
check("V1d getTokensOut(WETH->TOKEN, 222890894906943052)", tokensOut(p, WETH, TOKEN, 222890894906943052n), ETHER / 10n);

// Ladder — Arbitrage.ts:19-29
const TEST_VOLUMES = [ETHER / 100n, ETHER / 10n, ETHER / 6n, ETHER / 4n, ETHER / 2n, ETHER, ETHER * 2n, ETHER * 5n, ETHER * 10n];
// getBestCrossedMarket — Arbitrage.ts:31-67 (crossed[0]=sellTo, crossed[1]=buyFrom)
function getBestCrossedMarket(crossedMarket) {
  let best;
  const sellToMarket = crossedMarket[0];
  const buyFromMarket = crossedMarket[1];
  for (const size of TEST_VOLUMES) {
    const mid = tokensOut(buyFromMarket, WETH, TOKEN, size);
    const proceeds = tokensOut(sellToMarket, TOKEN, WETH, mid);
    const profit = proceeds - size;
    if (best !== undefined && profit < best.profit) {
      const trySize = (size + best.volume) / 2n; // meet halfway, Arbitrage.ts:42
      const tryMid = tokensOut(buyFromMarket, WETH, TOKEN, trySize);
      const tryProceeds = tokensOut(sellToMarket, TOKEN, WETH, tryMid);
      const tryProfit = tryProceeds - trySize;
      if (tryProfit > best.profit) best = { volume: trySize, profit: tryProfit };
      break; // Arbitrage.ts:55
    }
    best = { volume: size, profit };
  }
  return best;
}

// V2 — test/Arbitrage.test.ts:21-32 ("Calculate Crossed Markets")
const m0 = mkPool([TOKEN, WETH], [ETHER, ETHER * 2n]);
const m1 = mkPool([TOKEN, WETH], [ETHER, ETHER]);
const best = getBestCrossedMarket([m0, m1]);
check("V2 volume", best.volume, 208333333333333333n);
check("V2 profit", best.profit, 0x012be1d487a428cen);

// V3 — test/Arbitrage.test.ts:33-43 (control negativo: reservas iguales => profit < 0)
const e0 = mkPool([TOKEN, WETH], [ETHER, ETHER]);
const e1 = mkPool([TOKEN, WETH], [ETHER, ETHER]);
const nc = getBestCrossedMarket([e0, e1]);
check("V3 no-cross profit < 0", nc.profit < 0n ? 1n : 0n, 1n);

if (failures > 0) { console.error(`${failures} vector(es) FALLARON`); process.exit(1); }
console.log("TODOS los vectores REA-1 coinciden con el upstream");
