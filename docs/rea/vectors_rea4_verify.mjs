// Vectores DERIVADOS de FICHA_REA4_ATOMIC.md — forma cerrada getAmountIn de blind-arb (BigInt puro)
// Fuente: flashbots/simple-blind-arbitrage@73c6c270b27059e6dc0469a6c08e0f7739e5d531
//   src/BlindBackrunLogic.sol:116-206 (getAmountIn/getNumerator/getDenominator), 233-242 (sqrt), 249-258 (getAmountOut)
// Inputs: test/blindBackrunMainnet.t.sol:278-317 (getFakePairData isWETHZero=false / getFlippedFakePairData true)
// NOTA: el upstream loguea amountIn con console.log SIN assert -> este valor es VECTOR DERIVADO, no upstream.
// Ejecutar: node docs/rea/vectors_rea4_verify.mjs  (exit 0 si todas las verificaciones coinciden)
let failures = 0;
function check(name, got, expected) {
  const ok = got === expected;
  if (!ok) { failures++; console.error(`FAIL ${name}: got ${got}, expected ${expected}`); }
  else console.log(`ok   ${name} = ${got}`);
}
function isqrt(x) { if (x === 0n) return 0n; let z = (x + 1n) / 2n, y = x; while (z < y) { y = z; z = (x / z + z) / 2n; } return y; } // L233-242
function getAmountOut(amountIn, reserveIn, reserveOut) { const aif = amountIn * 997n; return aif * reserveOut / (reserveIn * 1000n + aif); } // L249-258
const F = 997n;
// Rama isWETHZero=false (L155-204) y rama true (L136-191), transcritas LITERALMENTE (sin generalizar):
//   true:  presqrt = F²·rA1·rB0/(rB1·rA0); num = (isqrt−1000)·rB1·rA0; den = F·rB1·1000 + F²·rA1
//   false: presqrt = F²·rA0·rB1/(rB0·rA1); num = (isqrt−1000)·rB0·rA1; den = F·rB0·1000 + F²·rA0
function closedFormTrue(rA0, rA1, rB0, rB1) {
  const presqrt = F * F * rA1 * rB0 / rB1 / rA0;
  const numerator = (isqrt(presqrt) - 1000n) * rB1 * rA0;
  const denominator = F * rB1 * 1000n + F * F * rA1;
  return { presqrt, isqrt: isqrt(presqrt), amountIn: numerator * 1000n / denominator };
}
function closedFormFalse(rA0, rA1, rB0, rB1) {
  const presqrt = F * F * rA0 * rB1 / rB0 / rA1;
  const numerator = (isqrt(presqrt) - 1000n) * rB0 * rA1;
  const denominator = F * rB0 * 1000n + F * F * rA0;
  return { presqrt, isqrt: isqrt(presqrt), amountIn: numerator * 1000n / denominator };
}
// test L299-317 (isWETHZero=false)
const v1 = closedFormFalse(17221979511711000000000n, 9022829950419911882261n, 24221870080988000000000n, 29260889455340067009671n);
// test L278-296 (isWETHZero=true, flipped)
const v2 = closedFormTrue(9022829950419911882261n, 17221979511711n, 29260889455340067009671n, 24221870080988n);
check("presqrt rama false", v1.presqrt, 2291978n);
check("presqrt rama true (espejo)", v2.presqrt, 2291978n);
check("isqrt(presqrt)", v1.isqrt, 1513n);
check("amountIn rama false (DERIVADO)", v1.amountIn, 2716778998396236163624n);
check("amountIn rama true (DERIVADO, simetria)", v2.amountIn, 2716778998396236163624n);
// trace flipped: pata1 getAmountOut(amountIn, first.r0, first.r1); pata2 getAmountOut(out1, second.r1, second.r0) (L67-68)
const out1 = getAmountOut(v2.amountIn, 9022829950419911882261n, 17221979511711n);
const out2 = getAmountOut(out1, 24221870080988n, 29260889455340067009671n);
check("out1 pata 1 (DERIVADO)", out1, 3976312652586n);
check("out2 pata 2 (DERIVADO)", out2, 4115530729036751327899n);
check("profit = out2 - amountIn (DERIVADO)", out2 - v2.amountIn, 1398751730640515164275n);
if (failures > 0) { console.error(`${failures} vector(es) FALLARON`); process.exit(1); }
console.log("TODOS los vectores REA-4 (derivados) coinciden con la formula cerrada del upstream");
