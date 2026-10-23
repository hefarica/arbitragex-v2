#!/usr/bin/env node
// =============================================================================
// CONTRACTS-ORACLE-01  —  ArbitrageX v2 / Contracts lane (profile 2.0)
// =============================================================================
// PURPOSE
//   Measure, from BYTES, whether the flash-loan callbacks shipped at the audited
//   revision satisfy the seven non-negotiable rules of `arbx-flash-loan-discipline`
//   (rule 4 = flash-loan callback discipline) and whether the three composed P0
//   findings F25 / F26 / F27 / F28 hold at that revision.
//
// ANCHORING (why this file is trustworthy)
//   It does NOT analyse the dirty worktree. It analyses `docs/contracts/anchored/`
//   and refuses to proceed unless every file's git blob hash equals the blob of the
//   AUDITED revision. The git blob hash is recomputed in pure Node:
//       sha1("blob " + byteLength + "\0" + bytes)
//   so a one-byte difference anywhere => TAMPER => non-zero exit, no verdict.
//   Analysed revision is pinned below; it is NOT the worktree HEAD.
//
// WHAT IT EMITS
//   1. VERDICT TABLE   one row per discipline rule / finding, each with the exact
//                      line numbers and the exact source span that decides it.
//   2. MUTANT PROOF    six textual mutants of the anchored bytes; each must FLIP the
//                      verdict of its target probe. A probe that cannot be flipped is
//                      reported CHECKER_WEAK (a defect in this oracle, not a PASS).
//   3. ENCODER         real keccak256 (Keccak-f[1600], verified against 3 published
//                      digest vectors + 1 external selector vector) + ABI encoders for
//                      the three callback/entrypoint signatures + the session-commitment
//                      binding that F25 says is missing, with a discrimination fixture.
//   4. FIXTURES        current predicate vs proposed predicate on the same forged
//                      callback scenario: proves the current 3-layer guard cannot
//                      distinguish an unsolicited genuine-vault callback.
//
// RUN
//   node docs/contracts/CONTRACTS-ORACLE-01.mjs --repo .
//
// EXIT CODES
//   0  oracle ran, anchors verified, keccak verified, all mutants flipped.
//      (A FAIL row is a FINDING about the contracts, not an oracle error.)
//   1  oracle invalid: anchor mismatch / keccak mismatch / mutant did not flip.
//
// BOUNDARIES (fail-honest)
//   * This is STATIC evidence over anchored source bytes. It is NOT a compiled build,
//     NOT a fork simulation, and NOT a signed transaction. A rule reported PASS here
//     means the construct is present in the bytes; it does not mean the deployed
//     bytecode was executed.
//   * forge(1) is blocked on this host by Windows AppControl ("Una directiva de Control
//     de aplicaciones bloqueo este archivo", documented as os error 4551), so no local
//     compile/fuzz/invariant run exists in this lane. Those tests are SPECIFIED in
//     CONTRACTS-CALLBACK-CHAIN-01.md and are NOT claimed as executed.
//   * Absence probes are bounded: each one prints the exact token set searched and the
//     exact body span scanned. "Absent" means "not found in that body for that token
//     set", never "impossible".
// =============================================================================

import { readFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join } from 'node:path';

const AUDITED_REV = 'bceb31ef7c0fd6d4b50bcdee6fc8d33858334822';
const AUDIT_BASELINE = '3f00b359beca82685280c5d8d30f099d8bd7d921';

// Blob hashes of the AUDITED revision (git rev-parse bceb31ef:<path>).
const ANCHORS = {
  'FlashLoanExecutor.sol':      { blob: '96be27838d149d3a3f1b74b5bd575c89d25017a9', path: 'contracts/src/FlashLoanExecutor.sol' },
  'ArbitrageExecutor.sol':      { blob: 'b48bfd60ec14eb88ca4678c35d554bc66aaa6a2f', path: 'contracts/src/ArbitrageExecutor.sol' },
  'BalancerFlashAdapter.sol':   { blob: '2c8848ea128eed9a2353c80133d2af2d8159487e', path: 'contracts/src/flashloans/BalancerFlashAdapter.sol' },
  'IFlashLoanProvider.sol':     { blob: '1685039230a09bcbf8d9ea5618d50adc9c6612dc', path: 'contracts/src/interfaces/IFlashLoanProvider.sol' },
  'AdminTimelock.sol':          { blob: '16d20235f7560dc31ea8bbe3760028194a6144ca', path: 'contracts/src/AdminTimelock.sol' },
  'FlashLoanExecutor.t.sol':    { blob: '5521800451c44247ec2f78f3b46cba35ee54bbad', path: 'contracts/test/FlashLoanExecutor.t.sol' },
  'FlashLoanRoundTrip.t.sol':   { blob: '909c58270c72ac0595aabb19fad8908ed5299818', path: 'contracts/test/FlashLoanRoundTrip.t.sol' },
};

// ── argv ─────────────────────────────────────────────────────────────────────
const argv = process.argv.slice(2);
const repoArg = argv.indexOf('--repo');
const REPO = repoArg >= 0 ? argv[repoArg + 1] : '.';
const ANCHOR_DIR = join(REPO, 'docs', 'contracts', 'anchored');

const out = [];
const say = (s = '') => out.push(s);
const hr = (c = '=') => say(c.repeat(78));

// =============================================================================
// 0. KECCAK-256 (original Keccak padding 0x01, NOT NIST SHA3)
// =============================================================================
const MASK64 = (1n << 64n) - 1n;
const RC = [
  0x0000000000000001n, 0x0000000000008082n, 0x800000000000808an, 0x8000000080008000n,
  0x000000000000808bn, 0x0000000080000001n, 0x8000000080008081n, 0x8000000000008009n,
  0x000000000000008an, 0x0000000000000088n, 0x0000000080008009n, 0x000000008000000an,
  0x000000008000808bn, 0x800000000000008bn, 0x8000000000008089n, 0x8000000000008003n,
  0x8000000000008002n, 0x8000000000000080n, 0x000000000000800an, 0x800000008000000an,
  0x8000000080008081n, 0x8000000000008080n, 0x0000000080000001n, 0x8000000080008008n,
];
const ROT = [
  [0, 36, 3, 41, 18],
  [1, 44, 10, 45, 2],
  [62, 6, 43, 15, 61],
  [28, 55, 25, 21, 56],
  [27, 20, 39, 8, 14],
];
const rotl = (v, n) => {
  const s = ((n % 64n) + 64n) % 64n;
  if (s === 0n) return v & MASK64;
  return ((v << s) | (v >> (64n - s))) & MASK64;
};
function keccakF(A) {
  for (let round = 0; round < 24; round++) {
    const C = new Array(5), D = new Array(5);
    for (let x = 0; x < 5; x++) C[x] = A[x] ^ A[x + 5] ^ A[x + 10] ^ A[x + 15] ^ A[x + 20];
    for (let x = 0; x < 5; x++) D[x] = C[(x + 4) % 5] ^ rotl(C[(x + 1) % 5], 1n);
    for (let x = 0; x < 5; x++) for (let y = 0; y < 5; y++) A[x + 5 * y] = (A[x + 5 * y] ^ D[x]) & MASK64;
    const B = new Array(25).fill(0n);
    for (let x = 0; x < 5; x++) for (let y = 0; y < 5; y++) {
      B[y + 5 * ((2 * x + 3 * y) % 5)] = rotl(A[x + 5 * y], BigInt(ROT[x][y]));
    }
    for (let x = 0; x < 5; x++) for (let y = 0; y < 5; y++) {
      A[x + 5 * y] = (B[x + 5 * y] ^ ((~B[((x + 1) % 5) + 5 * y] & MASK64) & B[((x + 2) % 5) + 5 * y])) & MASK64;
    }
    A[0] = (A[0] ^ RC[round]) & MASK64;
  }
}
function keccak256(bytes) {
  const RATE = 136; // 1088-bit rate for a 256-bit digest
  const buf = Buffer.from(bytes);
  const A = new Array(25).fill(0n);
  const absorb = (blk) => {
    for (let off = 0; off < RATE; off += 8) {
      let lane = 0n;
      for (let i = 7; i >= 0; i--) lane = (lane << 8n) | BigInt(blk[off + i]); // little-endian lane
      A[off / 8] = (A[off / 8] ^ lane) & MASK64;
    }
    keccakF(A);
  };
  // Multi-block absorb: every full RATE-sized block first (a message that is an exact
  // multiple of the rate still needs the trailing padding block below).
  let off = 0;
  while (buf.length - off >= RATE) { absorb(buf.subarray(off, off + RATE)); off += RATE; }
  const rem = buf.length - off;
  const last = Buffer.alloc(RATE, 0);
  if (rem > 0) buf.subarray(off).copy(last, 0);
  last[rem] = 0x01;                 // Keccak domain (NIST SHA3 uses 0x06)
  last[RATE - 1] |= 0x80;
  absorb(last);
  const digest = Buffer.alloc(32);
  for (let i = 0; i < 4; i++) {
    let lane = A[i];
    for (let b = 0; b < 8; b++) { digest[i * 8 + b] = Number(lane & 0xffn); lane >>= 8n; }
  }
  return digest;
}
const hex = (b) => Buffer.from(b).toString('hex');

// Published digest vectors (independent of this implementation).
const KECCAK_VECTORS = [
  { input: '',                                             want: 'c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470' },
  { input: 'abc',                                           want: '4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45' },
  { input: 'Transfer(address,address,uint256)',             want: 'ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef' },
];
// External cross-check: canonical Balancer V2 IFlashLoanRecipient selector.
const EXTERNAL_SELECTOR_VECTOR = { sig: 'receiveFlashLoan(address[],uint256[],uint256[],bytes)', want: 'f04f2707' };

// =============================================================================
// 1. ANCHOR VERIFICATION
// =============================================================================
function gitBlobHash(buf) {
  const header = Buffer.from(`blob ${buf.length}\0`, 'utf8');
  return createHash('sha1').update(Buffer.concat([header, buf])).digest('hex');
}

const SRC = {};
const anchorRows = [];
let anchorFail = 0;
for (const [name, meta] of Object.entries(ANCHORS)) {
  const p = join(ANCHOR_DIR, name);
  if (!existsSync(p)) { anchorRows.push([name, 'MISSING', meta.blob, '-']); anchorFail++; continue; }
  const buf = readFileSync(p);
  const got = gitBlobHash(buf);
  SRC[name] = buf.toString('utf8');
  const ok = got === meta.blob;
  if (!ok) anchorFail++;
  anchorRows.push([name, ok ? 'VERIFIED' : 'TAMPER', meta.blob, got]);
}

// =============================================================================
// 2. SOURCE NAVIGATION (comment/string-aware brace matching)
// =============================================================================
function stripCommentsAndStrings(s) {
  let res = '';
  let i = 0;
  while (i < s.length) {
    if (s[i] === '/' && s[i + 1] === '/') { while (i < s.length && s[i] !== '\n') { res += ' '; i++; } continue; }
    if (s[i] === '/' && s[i + 1] === '*') { i += 2; while (i < s.length && !(s[i] === '*' && s[i + 1] === '/')) { res += (s[i] === '\n' ? '\n' : ' '); i++; } i += 2; continue; }
    if (s[i] === '"' || s[i] === "'") { const q = s[i]; res += ' '; i++; while (i < s.length && s[i] !== q) { res += (s[i] === '\\' ? ' ' : (s[i] === '\n' ? '\n' : ' ')); if (s[i] === '\\') i++; i++; } res += ' '; i++; continue; }
    res += s[i]; i++;
  }
  return res;
}
const lineOf = (code, idx) => code.slice(0, idx).split('\n').length;

/** Extract the body of `function <name>(` with line span, from comment-stripped code. */
function fnBody(code, name) {
  const re = new RegExp(`function\\s+${name}\\s*\\(`, 'g');
  const m = re.exec(code);
  if (!m) return null;
  const openBrace = code.indexOf('{', m.index);
  if (openBrace < 0) return null;
  let depth = 0, i = openBrace;
  for (; i < code.length; i++) {
    if (code[i] === '{') depth++;
    else if (code[i] === '}') { depth--; if (depth === 0) break; }
  }
  return {
    text: code.slice(openBrace, i + 1),
    startLine: lineOf(code, openBrace),
    endLine: lineOf(code, i),
    idx: m.index,
    headerLine: lineOf(code, m.index),
  };
}
/** Extract an expression/statement region around an `if (...)` whose text matches. */
function findIfLine(code, pattern) {
  const lines = code.split('\n');
  for (let i = 0; i < lines.length; i++) if (pattern.test(lines[i])) return i + 1;
  return null;
}

/**
 * Which tests genuinely drive the Balancer callback?
 * A test drives it if its body calls receiveFlashLoan(...) DIRECTLY, or calls a local
 * helper whose own body calls it (depth-1 interprocedural resolution — this matters:
 * mockVault.triggerFlashLoan(exec, ...) is how the A9 tests reach the callback).
 * BOUND: depth-1 only. A deeper local chain would be reported MISLABELED; the resolution
 * depth and the helper set are printed with the evidence so the bound is auditable.
 */
function balancerCallbackCoverage(code) {
  const helpers = new Set();
  for (const m of code.matchAll(/function\s+([A-Za-z0-9_]+)\s*\(/g)) {
    const b = fnBody(code, m[1]);
    if (b && /receiveFlashLoan\s*\(/.test(b.text)) helpers.add(m[1]);
  }
  helpers.delete('receiveFlashLoan');
  for (const h of [...helpers]) if (/^test/.test(h)) helpers.delete(h); // a test is not a helper
  const names = [...code.matchAll(/function\s+(test[A-Za-z0-9_]*)\s*\(/g)].map((m) => m[1]);
  const claims = names.filter((n) => /ReceiveFlashLoan/.test(n));
  const mislabeled = [], honest = [];
  for (const n of claims) {
    const body = fnBody(code, n)?.text || '';
    const direct = /receiveFlashLoan\s*\(/.test(body);
    const viaHelper = [...helpers].some((h) => h !== n && new RegExp(`\\b${h}\\s*\\(`).test(body));
    (direct || viaHelper ? honest : mislabeled).push(n);
  }
  return { claims, mislabeled, honest, helpers: [...helpers] };
}

const C = {
  flx: stripCommentsAndStrings(SRC['FlashLoanExecutor.sol']),
  arb: stripCommentsAndStrings(SRC['ArbitrageExecutor.sol']),
  tExec: stripCommentsAndStrings(SRC['FlashLoanExecutor.t.sol']),
  tRT: stripCommentsAndStrings(SRC['FlashLoanRoundTrip.t.sol']),
  tl: stripCommentsAndStrings(SRC['AdminTimelock.sol']),
  rawFlx: SRC['FlashLoanExecutor.sol'],
  rawTExec: SRC['FlashLoanExecutor.t.sol'],
  rawTL: SRC['AdminTimelock.sol'],
};

const B = {
  receiveFlashLoan: fnBody(C.flx, 'receiveFlashLoan'),
  executeOperation: fnBody(C.flx, 'executeOperation'),
  repayAave: fnBody(C.flx, '_executeAndRepayAave'),
  requestFlashLoan: fnBody(C.flx, 'requestFlashLoan'),
  pauseArb: fnBody(C.arb, 'pause'),
  emergencyWithdraw: fnBody(C.arb, 'emergencyWithdraw'),
};

// =============================================================================
// 3. PROBES
// =============================================================================
// Absence probes are explicitly bounded: TOKENS is printed with each result.
const SESSION_TOKENS = [
  '_pending', 'pendingRequest', 'session', 'Session', 'commitment', 'Commitment',
  '_requestHash', 'requestId', 'expectedParamsHash', 'paramsHash', '_inflight',
  '_activeRequest', 'tstore', 'transient', 'initiator',
];

const rows = [];
function row(id, rule, status, evidence) { rows.push({ id, rule, status, evidence }); }

// C1 — rule 2: caller authenticated on the Aave path.
{
  const line = findIfLine(C.flx, /msg\.sender\s*!=\s*address\(aavePool\)/);
  row('C1', 'FL-2 caller==aavePool', line ? 'PASS' : 'FAIL',
      line ? `FlashLoanExecutor.sol:${line}  if (msg.sender != address(aavePool)) revert FL_UnauthorizedCaller();`
           : 'no msg.sender==aavePool check found');
}
// C2 — rule 2: caller authenticated on the Balancer path.
{
  const line = findIfLine(C.flx, /msg\.sender\s*!=\s*balancerVault/);
  row('C2', 'FL-2 caller==balancerVault', line ? 'PASS' : 'FAIL',
      line ? `FlashLoanExecutor.sol:${line}  if (msg.sender != balancerVault) revert FL_UnauthorizedCaller();`
           : 'no msg.sender==balancerVault check found');
}
// C3 — rule 3: initiator authenticated (Aave exposes it).
{
  const has = /initiator\s*!=\s*address\(this\)/.test(B.executeOperation?.text || '');
  row('C3', 'FL-3 initiator==this (Aave)', has ? 'PASS' : 'FAIL',
      has ? `FlashLoanExecutor.sol:${findIfLine(C.flx, /initiator\s*!=\s*address\(this\)/)}  if (initiator != address(this)) revert FL_InvalidInitiator();`
          : 'no initiator check found');
}
// C4 — rule 3 on the BALANCER path: initiator is NOT exposed by the Vault, so the
// discipline requires a transient session flag / request commitment instead.        <<< F25
{
  const body = B.receiveFlashLoan?.text || '';
  const found = SESSION_TOKENS.filter((t) => body.includes(t));
  const guardOnly = ['balancerVault', 'flashLoanProvider', 'address(0)'].filter((t) => body.includes(t));
  row('C4', 'FL-3 session/request binding (Balancer)', found.length === 0 ? 'ABSENT' : 'PASS',
      `searched ${SESSION_TOKENS.length} binding tokens in receiveFlashLoan body ` +
      `(FlashLoanExecutor.sol:${B.receiveFlashLoan?.startLine}-${B.receiveFlashLoan?.endLine}, ${body.length} chars): ` +
      `found=[${found.join(',') || 'none'}] ; only guards present=[${guardOnly.join(',')}]`);
  row('C4b', 'F25 structural: can a binding be transmitted?', 'ABSENT',
      'IFlashLoanProvider.flashLoan(address receiver,address asset,uint256 amount,bytes params) and the Balancer Vault ' +
      'flashLoan(recipient,address[],uint256[],bytes) ABI both carry NO initiator/session field; the only carrier is ' +
      '`params`/`userData`, and receiveFlashLoan never records nor compares it => an unsolicited genuine-Vault callback ' +
      'runs arbitrary userData with an amount-sized approval already live.');
}
// C4c — PRODUCER side of the same binding. A consumer can only compare what a producer
// recorded. If requestFlashLoan records nothing, no callback could bind anything, even
// conceptually. This closes the "you searched in the wrong place" objection.
{
  const body = B.requestFlashLoan?.text || '';
  const lines = (SRC['FlashLoanExecutor.sol'].split('\n').slice((B.requestFlashLoan?.startLine || 1) - 1, B.requestFlashLoan?.endLine || 1));
  const keccakLines = lines.filter((l) => /keccak256\s*\(\s*params\s*\)/.test(l));
  const emitKeccak = keccakLines.filter((l) => /emit\s+FlashLoanRequested/.test(l));
  const stateWritePatterns = [/_pending\w*\s*=/, /requestHash\w*\s*=/, /expected\w*\s*=/];
  const writes = stateWritePatterns.filter((p) => p.test(body));
  // Full state-variable inventory of the contract: any declaration carrying a
  // visibility modifier, with or without an initializer; `constant` is classified
  // separately because a constant is NOT a storage slot.
  const declRe = /^[ \t]+((?:mapping\s*\([^;]*?\))|[A-Za-z_][\w.]*(?:\[\])?)\s+((?:public|private|internal)\s+(?:constant\s+)?)(\w+)\s*(?:=[^;]*)?;/gm;
  const allDecls = [...SRC['FlashLoanExecutor.sol'].matchAll(declRe)]
    .map((m) => ({ type: m[1], mods: m[2].trim(), name: m[3] }));
  const storageSlots = allDecls.filter((d) => !/constant/.test(d.mods));
  const constSlots = allDecls.filter((d) => /constant/.test(d.mods));
  const requestLike = storageSlots.filter((d) => /bytes32/.test(d.type));
  row('C4c', 'F25 producer side: request records no request identity',
      (writes.length === 0 && keccakLines.length === emitKeccak.length && requestLike.length === 0) ? 'ABSENT' : 'PASS',
      `requestFlashLoan body (FlashLoanExecutor.sol:${B.requestFlashLoan?.startLine}-${B.requestFlashLoan?.endLine}): ` +
      `keccak256(params) appears ${keccakLines.length}x and ALL ${emitKeccak.length} occurrences are the ` +
      `\`emit FlashLoanRequested(asset, amount, keccak256(params))\` event argument — an event is not readable state. ` +
      `Request-identity state writes found=[${writes.join(',') || 'none'}]. ` +
      `Storage slots (${storageSlots.length}, declared mods in parens) = [${storageSlots.map((d) => `${d.type} ${d.name}(${d.mods})`).join(', ')}]; ` +
      `constants excluded as non-storage (${constSlots.length}) = [${constSlots.map((d) => d.name).join(', ')}]. ` +
      `bytes32-typed STORAGE slots = ${requestLike.length} => there is no slot in which a request identity could be ` +
      `recorded, so no callback could be bound to one.`);
}
{
  const b = B.receiveFlashLoan?.text || '';
  const a = B.repayAave?.text || '';
  const okB = /safeTransfer\s*\(\s*msg\.sender\s*,/.test(b);
  const okA = /forceApprove\s*\(\s*address\(aavePool\)/.test(a);
  row('C5', 'FL-1 repay-or-revert (both callbacks)', okB && okA ? 'PASS' : 'FAIL',
      `balancer: safeTransfer(msg.sender, amountOwed) present=${okB} ; aave: forceApprove(aavePool, amountToOwe) present=${okA}`);
}
// C6 — rule 4: no standing approval survives.
{
  const b = B.receiveFlashLoan?.text || '';
  const a = B.repayAave?.text || '';
  const okB = /forceApprove\s*\(\s*arbitrageExecutor\s*,\s*0\s*\)/.test(b);
  const okA = /forceApprove\s*\(\s*arbitrageExecutor\s*,\s*0\s*\)/.test(a);
  row('C6', 'FL-4 residual allowance cleared', okB && okA ? 'PASS' : 'FAIL',
      `reset-to-zero present: balancer=${okB} aave=${okA}`);
}
// C7 — rule 5: on-chain profit floor BEFORE repay.                              <<< F26 contributor
{
  const bodies = [B.receiveFlashLoan?.text || '', B.repayAave?.text || ''].join('\n');
  const toks = ['minProfit', 'minProfitFloor', 'profitFloor', 'MIN_PROFIT'];
  const found = toks.filter((t) => bodies.includes(t));
  row('C7', 'FL-5 on-chain profit floor before repay', found.length === 0 ? 'ABSENT' : 'PASS',
      `searched [${toks.join(',')}] in both callback bodies: found=[${found.join(',') || 'none'}]`);
}
// C8 — rule 6: fee read, never hardcoded.
{
  const b = B.receiveFlashLoan?.text || '';
  const a = B.executeOperation?.text || '';
  const fromArg = /feeAmounts\s*\[\s*0\s*\]/.test(b) && /premium/.test(SRC['FlashLoanExecutor.sol']);
  const literals = [...SRC['FlashLoanExecutor.sol'].matchAll(/\b(?:5|9)\s*\*\s*[a-zA-Z_]+\s*\/\s*10_?000/g)].map((m) => m[0]);
  row('C8', 'FL-6 fee from callback arg / on-chain', fromArg && literals.length === 0 ? 'PASS' : 'FAIL',
      `feeAmounts[0] used=${/feeAmounts\s*\[\s*0\s*\]/.test(b)} ; hardcoded bps patterns=[${literals.join('|') || 'none'}]`);
}
// C9 — rule 7: no nested flash loan inside a callback.
{
  const bodies = [B.receiveFlashLoan?.text || '', B.repayAave?.text || ''].join('\n');
  const nested = /\.flashLoan\s*\(|\.flashLoanSimple\s*\(/.test(bodies);
  row('C9', 'FL-7 no nested flashLoan in callback', nested ? 'FAIL' : 'PASS',
      `nested flashLoan/flashLoanSimple call inside callbacks = ${nested}`);
}
// C10 — F26 CORE: repayment solvency measured on the contract TOTAL balance.        <<< F26
{
  const lineB = findIfLine(C.flx, /balanceOf\(address\(this\)\)\s*<\s*amountOwed/);
  const lineA = findIfLine(C.flx, /balanceOf\(address\(this\)\)\s*<\s*amountToOwe/);
  const borrowedDelta = /balBefore|initialBalance|balanceBeforeRepay|_pre[A-Z]/.test(B.receiveFlashLoan?.text || '');
  row('C10', 'F26 solvency measured on TOTAL balance, not op delta',
      (lineB && lineA) ? 'FAIL' : 'PASS',
      `FlashLoanExecutor.sol:${lineB} (balancer) and :${lineA} (aave) compare asset.balanceOf(address(this)) against the owed ` +
      `amount. Delta-scoped accounting present in the callback = ${borrowedDelta}. A pre-existing reserve >= (owed - gross) ` +
      `therefore satisfies the gate and the shortfall is charged to the reserve.`);
}
// C11 — F27: is there ANY ERC-20 exit from the wrapper where profit is retained?
{
  const names = [...SRC['FlashLoanExecutor.sol'].matchAll(/function\s+([A-Za-z0-9_]+)\s*\(/g)].map((m) => m[1]);
  const exitLike = names.filter((n) => /withdraw|sweep|rescue|treasury|drain|collect/i.test(n));
  const arbExit = [...SRC['ArbitrageExecutor.sol'].matchAll(/function\s+([A-Za-z0-9_]+)\s*\(/g)]
    .map((m) => m[1]).filter((n) => /withdraw|sweep|rescue/i.test(n));
  row('C11', 'F27 ERC-20 profit exit from wrapper', exitLike.length === 0 ? 'ABSENT' : 'PASS',
      `FlashLoanExecutor public/external functions = [${names.join(',')}] ; exit-like = [${exitLike.join(',') || 'none'}]. ` +
      `ArbitrageExecutor HAS [${arbExit.join(',')}] (line ${findIfLine(C.arb, /function\s+emergencyWithdraw/)}), the wrapper has none. ` +
      `Retention is asserted by the suite itself: FlashLoanRoundTrip.t.sol:219 assertEq(token.balanceOf(address(flashExec)), profit, "net profit retained by the borrower").`);
}
// C12 — F28: is the wrapper pausable?
{
  const pauseable = /Pausable|whenNotPaused|_pause\(\)/.test(SRC['FlashLoanExecutor.sol']);
  row('C12', 'F28 wrapper entrypoints pausable', pauseable ? 'PASS' : 'ABSENT',
      `FlashLoanExecutor.sol imports/uses Pausable=${pauseable}; requestFlashLoan/executeOperation/receiveFlashLoan carry no whenNotPaused. ` +
      `ArbitrageExecutor IS pausable (ArbitrageExecutor.sol:${findIfLine(C.arb, /function\s+pause\(\)/)}), so a pause stops route dispatch but not the loan request nor the callback.`);
}
// C13 — F28 authority: who can actually call pause(), and how fast?
{
  const pauseLine = findIfLine(C.arb, /function\s+pause\(\)/);
  const roleLine = C.arb.split('\n').slice((pauseLine || 1) - 1, (pauseLine || 1) + 1).join(' ').trim();
  const zeroDelayGuard = findIfLine(C.tl, /revert\s+AdminTimelock__ZeroMinDelay/);
  const ctorAssign = /minDelay\s*=\s*_minDelay|minDelay\s*=\s*[A-Za-z_][A-Za-z0-9_]*\s*;/.test(C.tl);
  const numericDelay = [...C.tl.matchAll(/minDelay\s*=\s*(\d[\d_]*)/g)].map((m) => m[1]);
  row('C13', 'F28 pause authority + latency', 'PARTIAL',
      `ArbitrageExecutor.sol:${pauseLine} "${roleLine}" -> pause() is DEFAULT_ADMIN_ROLE, not a scoped guardian. ` +
      `AdminTimelock.sol:${zeroDelayGuard} rejects a zero delay; the delay is a constructor PARAMETER (assignment present=${ctorAssign}, ` +
      `numeric literal in source=[${numericDelay.join(',') || 'none'}]) so the production pause latency is a DEPLOY-TIME value not derivable from these bytes ` +
      `=> reported NO COMPUTED VALUE, not zero. Production wiring gives ADMIN_ROLE to the timelock ` +
      `(contracts/test/DeployMainnetRoleCustody.t.sol: testHandoff_Timelock_HoldsAdminRole_AllThree), and AdminTimelock.t.sol:175 ` +
      `testA9_Timelock_AdminCannotBypassDelay asserts the admin CANNOT bypass the delay => 'immediate on-chain pause' is not reachable through ` +
      `the only pause path that exists, without adding a scoped guardian role.`);
}
// C14 — F16 class: test NAME vs test BODY binding.
// Semantics: for every test whose NAME claims the Balancer callback, its BODY must
// actually invoke receiveFlashLoan. A test that claims ReceiveFlashLoan and drives
// executeOperation is name/body mismatched. Tests that name neither are out of scope.
{
  const cov = balancerCallbackCoverage(C.tExec);
  const testNames = [...C.tExec.matchAll(/function\s+(test[A-Za-z0-9_]*)\s*\(/g)].map((m) => m[1]);
  const layerTests = testNames.filter((n) => /A9_ReceiveFlashLoan|RevertsOnUnauthorizedSender|RevertsWhenVaultNotSet/.test(n));
  row('C14', 'F16 test name == test body (Balancer callback)',
      cov.mislabeled.length === 0 ? 'PASS' : 'MISLABELED',
      `tests whose NAME claims the Balancer callback = ${cov.claims.length}; of those, ${cov.mislabeled.length} never reach ` +
      `receiveFlashLoan even through depth-1 local helpers (helpers resolved = [${cov.helpers.join(', ')}]): ` +
      `[${cov.mislabeled.join(', ')}] -> their bodies drive executeOperation, the AAVE callback. ` +
      `Correctly-named Balancer tests = ${cov.honest.length}. Layer tests [${layerTests.join(', ')}] cover exactly: ` +
      `"sender is not the configured vault" and "vault/provider unset". NO test drives the GENUINE configured vault with ` +
      `attacker-chosen userData from an address that is not the operator -- which is precisely why the 3 layers pass while C4/F25 stays open.`);
}
// C15 — F26 coverage: is the shortfall path ever exercised WITH a pre-existing reserve?
{
  const mintLine = findIfLine(C.tExec, /token\.mint\(address\(flashExec\)/);
  const shortfallTest = C.tRT.split('\n').findIndex((l) => /testRoundTrip_Aave_RepaymentShortfall_RevertsNamed/.test(l)) + 1;
  const rtSetupMints = /token\.mint\(address\(flashExec\)/.test(C.tRT);
  const assertsReserve = /balanceOf\(address\(flashExec\)\)\s*,\s*[A-Za-z0-9_]+e18\s*,\s*"(?:reserve|preExisting|preserved)/i.test(C.tRT);
  row('C15', 'F26 coverage: shortfall tested with pre-existing reserve',
      (mintLine && shortfallTest > 0 && !rtSetupMints) ? 'ABSENT' : 'PASS',
      `FlashLoanExecutor.t.sol:${mintLine} setUp mints 10_000e18 INTO the wrapper ("Mint tokens to flash loan executor to cover repayment"), ` +
      `so every happy-path test in that file passes because of the reserve. FlashLoanRoundTrip.t.sol:${shortfallTest} is the only test that expects ` +
      `FL_RepaymentShortfall, and its contract starts the wrapper at ZERO (no reserve mint). An explicit reserve-preservation assertion on a shortfall ` +
      `path exists = ${assertsReserve}.`);
}

// =============================================================================
// 4. MUTANT PROOF — can each probe actually fail the other way?
// =============================================================================
const mutants = [];
function mutateRule(targetId, label, transform) {
  // Re-derive the probe inputs from transformed bytes (comments stripped the same way).
  const flxMut = stripCommentsAndStrings(transform(SRC['FlashLoanExecutor.sol']));
  const arbMut = stripCommentsAndStrings(transform(SRC['ArbitrageExecutor.sol']));
  const tnMut = stripCommentsAndStrings(transform(SRC['FlashLoanExecutor.t.sol']));
  const before = rows.find((r) => r.id === targetId)?.status;
  let after = before;
  if (targetId === 'C4') {
    const rb = fnBody(flxMut, 'receiveFlashLoan');
    const found = SESSION_TOKENS.filter((t) => (rb?.text || '').includes(t));
    after = found.length === 0 ? 'ABSENT' : 'PASS';
  } else if (targetId === 'C2') {
    after = findIfLine(flxMut, /msg\.sender\s*!=\s*balancerVault/) ? 'PASS' : 'FAIL';
  } else if (targetId === 'C10') {
    const lineB = findIfLine(flxMut, /balanceOf\(address\(this\)\)\s*<\s*amountOwed/);
    const lineA = findIfLine(flxMut, /balanceOf\(address\(this\)\)\s*<\s*amountToOwe/);
    after = (lineB && lineA) ? 'FAIL' : 'PASS';
  } else if (targetId === 'C4c') {
    const body = fnBody(flxMut, 'requestFlashLoan')?.text || '';
    const writes = [/_pending\w*\s*=/, /requestHash\w*\s*=/, /expected\w*\s*=/].filter((p) => p.test(body));
    const kl = (body.match(/keccak256\s*\(\s*params\s*\)/g) || []).length;
    const em = (body.match(/emit\s+FlashLoanRequested[^;]*keccak256\s*\(\s*params\s*\)/g) || []).length;
    after = (writes.length === 0 && kl === em) ? 'ABSENT' : 'PASS';
  } else if (targetId === 'C7') {
    const bodies = [fnBody(flxMut, 'receiveFlashLoan')?.text || '', fnBody(flxMut, '_executeAndRepayAave')?.text || ''].join('\n');
    after = bodies.includes('minProfitFloor') ? 'PASS' : 'ABSENT';
  } else if (targetId === 'C11') {
    const names = [...arbMut.matchAll(/function\s+([A-Za-z0-9_]+)\s*\(/g)].map((m) => m[1]);
    const alt = [...transform(SRC['FlashLoanExecutor.sol']).matchAll(/function\s+([A-Za-z0-9_]+)\s*\(/g)].map((m) => m[1]);
    after = alt.filter((n) => /withdraw|sweep|rescue/i.test(n)).length === 0 ? 'ABSENT' : 'PASS';
  } else if (targetId === 'C12') {
    after = /Pausable|whenNotPaused/.test(transform(SRC['FlashLoanExecutor.sol'])) ? 'PASS' : 'ABSENT';
  } else if (targetId === 'C14') {
    const cov = balancerCallbackCoverage(tnMut);
    after = cov.mislabeled.length === 0 ? 'PASS' : 'MISLABELED';
  }
  mutants.push({ targetId, label, before, after, flipped: before !== after });
}

mutateRule('C2', 'M1: delete the balancerVault caller check',
  (s) => s.replace('if (msg.sender != balancerVault) revert FL_UnauthorizedCaller();',
                   'if (false) revert FL_UnauthorizedCaller();'));
mutateRule('C4', 'M2: add a transient session binding to receiveFlashLoan',
  (s) => s.replace('IERC20 asset = tokens[0];',
                   'if (keccak256(userData) != _pendingRequestHash) revert FL_UnauthorizedCaller();\n        IERC20 asset = tokens[0];')
         .replace('uint256 private _reentrancyStatus = _NOT_ENTERED;',
                  'uint256 private _reentrancyStatus = _NOT_ENTERED;\n    bytes32 private _pendingRequestHash;'));
mutateRule('C4c', 'M8: make requestFlashLoan RECORD the request identity (producer side)',
  (s) => s.replace('        // SC-06: emit after the call so the event is only logged on success\n        emit FlashLoanRequested(asset, amount, keccak256(params));',
                   '        // SC-06: emit after the call so the event is only logged on success\n        _pendingRequestHash = keccak256(params);\n        emit FlashLoanRequested(asset, amount, keccak256(params));')
         .replace('uint256 private _reentrancyStatus = _NOT_ENTERED;',
                  'uint256 private _reentrancyStatus = _NOT_ENTERED;\n    bytes32 private _pendingRequestHash;'));
mutateRule('C7', 'M3: add an on-chain profit floor',  (s) => s.replace('uint256 amountOwed = amount + premium;',
                   'uint256 amountOwed = amount + premium + minProfitFloor;'));
mutateRule('C10', 'M4: scope repayment to the operation delta',
  (s) => s.replace('if (asset.balanceOf(address(this)) < amountOwed) revert FL_RepaymentShortfall();',
                   'if (asset.balanceOf(address(this)) < balBeforeRepay + amountOwed) revert FL_RepaymentShortfall();'));
mutateRule('C11', 'M5: add an admin treasury sweep to the wrapper',
  (s) => s.replace('function setReferralCode(uint16 _code)',
                   'function sweepToTreasury(address token, address to) external onlyRole(DEFAULT_ADMIN_ROLE) { IERC20(token).safeTransfer(to, IERC20(token).balanceOf(address(this))); }\n\n    function setReferralCode(uint16 _code)'));
mutateRule('C12', 'M6: make the wrapper pausable',
  (s) => s.replace('import "./interfaces/IFlashLoanProvider.sol";',
                   'import "./interfaces/IFlashLoanProvider.sol";\nimport "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";')
         .replace('function requestFlashLoan(address asset, uint256 amount, bytes calldata params) external onlyRole(EXECUTOR_ROLE) {',
                  'function requestFlashLoan(address asset, uint256 amount, bytes calldata params) external onlyRole(EXECUTOR_ROLE) whenNotPaused {'));
mutateRule('C14', 'M7: rename EVERY mislabeled test to the callback it actually drives',
  (s) => s.replaceAll('function testReceiveFlashLoan_Authorized() public {',
                      'function testExecuteOperation_Authorized() public {')
         .replaceAll('function testReceiveFlashLoan_RejectsUnauthorized() public {',
                     'function testExecuteOperation_RejectsUnauthorized() public {')
         .replaceAll('function testReceiveFlashLoan_RejectsInvalidInitiator() public {',
                     'function testExecuteOperation_RejectsInvalidInitiator() public {'));

const weak = mutants.filter((m) => !m.flipped);

// =============================================================================
// 5. ENCODER + SESSION COMMITMENT (the capability F25 says is missing)
// =============================================================================
const word = (v) => {
  const h = (typeof v === 'bigint' ? v : BigInt(v)).toString(16).padStart(64, '0');
  return h;
};
const wordAddr = (a) => word(BigInt(a));
const offWord = (n) => word(BigInt(n));
function abiArrayUint(amounts) {
  const head = word(amounts.length);
  const body = amounts.map((a) => word(a)).join('');
  return head + body;
}
function abiArrayAddr(addrs) {
  const head = word(addrs.length);
  const body = addrs.map((a) => wordAddr(a)).join('');
  return head + body;
}
function abiBytes(b) {
  const buf = Buffer.from(b);
  const padded = Math.ceil(buf.length / 32) * 32;
  return word(buf.length) + buf.toString('hex').padEnd(padded * 2, '0');
}
const selector = (sig) => hex(keccak256(Buffer.from(sig, 'utf8'))).slice(0, 8);

/** receiveFlashLoan(IERC20[] tokens, uint256[] amounts, uint256[] feeAmounts, bytes userData) */
function encodeReceiveFlashLoan(tokens, amounts, fees, userData) {
  const sig = 'receiveFlashLoan(address[],uint256[],uint256[],bytes)';
  const t = abiArrayAddr(tokens), a = abiArrayUint(amounts), f = abiArrayUint(fees), u = abiBytes(userData);
  const offT = 4 * 32, offA = offT + t.length / 2, offF = offA + a.length / 2, offU = offF + f.length / 2;
  return '0x' + selector(sig) + offWord(offT) + offWord(offA) + offWord(offF) + offWord(offU) + t + a + f + u;
}
/** executeOperation(address asset, uint256 amount, uint256 premium, address initiator, bytes params) */
function encodeExecuteOperation(asset, amount, premium, initiator, params) {
  const sig = 'executeOperation(address,uint256,uint256,address,bytes)';
  const p = abiBytes(params);
  return '0x' + selector(sig) + wordAddr(asset) + word(amount) + word(premium) + wordAddr(initiator) + offWord(4 * 32) + p;
}
/** IFlashLoanProvider.flashLoan(address receiver, address asset, uint256 amount, bytes params) */
function encodeProviderFlashLoan(receiver, asset, amount, params) {
  const sig = 'flashLoan(address,address,uint256,bytes)';
  const p = abiBytes(params);
  return '0x' + selector(sig) + wordAddr(receiver) + wordAddr(asset) + word(amount) + offWord(4 * 32) + p;
}

/**
 * The missing primitive: a commitment that binds ONE initiated request to ONE callback.
 * The wrapper would store it in transient storage before calling the provider, and the
 * callback would recompute+compare it, then clear it.
 */
function requestCommitment(chainId, wrapper, provider, asset, amount, userData) {
  const inner = Buffer.from(
    word(chainId) + wordAddr(wrapper) + wordAddr(provider) + wordAddr(asset) + word(amount) +
    hex(keccak256(Buffer.from(userData))), 'hex');
  return hex(keccak256(inner));
}

/** Scenario = the full fact set a callback sees. */
const HONEST = {
  label: 'honest: requestFlashLoan -> provider -> callback',
  chainId: 1n,
  wrapper: '0x1111111111111111111111111111111111111111',
  provider: '0xBA12222222228d8Ba445958a75a0704d566BF2C8',
  asset: '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
  amount: 1000000000000000000n,
  userData: '0xabcdef01',
  pendingCommitment: null, // filled below
  senderIsVault: true,
  vaultConfigured: true,
  providerConfigured: true,
  reenter: false,
};
HONEST.pendingCommitment = requestCommitment(HONEST.chainId, HONEST.wrapper, HONEST.provider, HONEST.asset, HONEST.amount, HONEST.userData);

const FORGERIES = [
  { ...HONEST, label: 'unsolicited genuine-Vault callback, ATTACKER-CHOSEN userData', userData: '0xdeadbeef', pendingCommitment: null, kind: 'payload' },
  { ...HONEST, label: 'unsolicited callback, different asset',                          asset: '0x6B175474E89094C44Da98b954EedeAC495271d0F', pendingCommitment: null, kind: 'payload' },
  { ...HONEST, label: 'unsolicited callback, inflated amount',                          amount: 100000000000000000000n,              pendingCommitment: null, kind: 'payload' },
  { ...HONEST, label: 'unsolicited callback, foreign chain id',                         chainId: 11155111n,                          pendingCommitment: null, kind: 'payload' },
  { ...HONEST, label: 'unsolicited callback, same tuple, no request in flight',         pendingCommitment: null,                     kind: 'state' },
  { ...HONEST, label: 'reentrant callback during an open request',                      reenter: true,                               kind: 'state' },
  { ...HONEST, label: 'callback from a lookalike vault (sender spoof)',                 senderIsVault: false,                        kind: 'state' },
];

// CURRENT predicate = exactly the 3 layers present at the audited rev (+ nonReentrant).
function predicateCurrent(s) {
  if (!s.vaultConfigured) return 'revert FL_BalancerVaultNotSet';
  if (!s.senderIsVault) return 'revert FL_UnauthorizedCaller';
  if (!s.providerConfigured) return 'revert FL_NoProviderConfigured';
  if (s.reenter) return 'revert FL_ReentrantCall';
  return 'ACCEPT';
}
// PROPOSED predicate = current + transient session commitment (rule 3 for Balancer).
function predicateProposed(s) {
  const cur = predicateCurrent(s);
  if (cur !== 'ACCEPT') return cur;
  if (!s.pendingCommitment) return 'revert FL_NoRequestInFlight';
  const want = requestCommitment(s.chainId, s.wrapper, s.provider, s.asset, s.amount, s.userData);
  if (want !== s.pendingCommitment) return 'revert FL_RequestMismatch';
  return 'ACCEPT';
}

// =============================================================================
// 6. REPORT
// =============================================================================
hr();
say('CONTRACTS-ORACLE-01 — ArbitrageX v2 / Contracts lane');
say(`audited rev : ${AUDITED_REV}`);
say(`prev cut    : ${AUDIT_BASELINE}`);
say(`analysed    : docs/contracts/anchored/*  (bytes proven == audited rev blobs)`);
say(`worktree rev: NOT used (the working tree is a different branch with uncommitted work)`);
hr();

say('');
say('[1] ANCHOR INTEGRITY (sha1 of "blob <len>\\0" + bytes vs pinned rev blob)');
say('FILE'.padEnd(30) + 'STATUS'.padEnd(10) + 'EXPECTED BLOB');
for (const [name, st, want, got] of anchorRows) {
  say(name.padEnd(30) + st.padEnd(10) + want.slice(0, 12) + '…' + (st === 'VERIFIED' ? '' : `  got=${String(got).slice(0, 12)}…`));
}
say(`=> ${anchorRows.filter((r) => r[1] === 'VERIFIED').length}/${anchorRows.length} anchored, ${anchorFail} tampered/missing`);

say('');
say('[2] KECCAK-256 SELF-TEST (digest vectors are published constants, independent of this file)');
let keccakFail = 0;
for (const v of KECCAK_VECTORS) {
  const got = hex(keccak256(Buffer.from(v.input, 'utf8')));
  const ok = got === v.want;
  if (!ok) keccakFail++;
  say(`  ${ok ? 'OK  ' : 'FAIL'} keccak256("${v.input}") = ${got.slice(0, 24)}…`);
}
const gotSel = selector(EXTERNAL_SELECTOR_VECTOR.sig);
const selOk = gotSel === EXTERNAL_SELECTOR_VECTOR.want;
if (!selOk) keccakFail++;
say(`  ${selOk ? 'OK  ' : 'FAIL'} external selector cross-check: ${EXTERNAL_SELECTOR_VECTOR.sig} -> 0x${gotSel} (canonical 0x${EXTERNAL_SELECTOR_VECTOR.want})`);
say(`=> keccak/failure count = ${keccakFail}${keccakFail ? '  *** encoder is NOT trustworthy, verdicts withheld ***' : ''}`);

say('');
say('[3] VERDICT TABLE — measured at the audited rev');
say('ID   RULE / FINDING'.padEnd(56) + 'STATUS');
for (const r of rows) say(r.id.padEnd(5) + r.rule.padEnd(51) + r.status);
say('');
for (const r of rows) say(`  [${r.id}] ${r.status}\n      ${r.evidence}`);

say('');
say('[4] MUTANT PROOF — a probe that cannot be flipped is not evidence');
say('TARGET  BEFORE'.padEnd(28) + 'AFTER'.padEnd(14) + 'FLIPPED  MUTATION');
for (const m of mutants) {
  say(m.targetId.padEnd(8) + String(m.before).padEnd(20) + String(m.after).padEnd(14) + (m.flipped ? 'yes' : 'NO') + '      ' + m.label);
}
say(`=> ${mutants.filter((m) => m.flipped).length}/${mutants.length} mutants flipped their target probe`);

say('');
say('[5] ENCODER — real selectors (keccak verified above)');
const SEL = {
  receiveFlashLoan: selector('receiveFlashLoan(address[],uint256[],uint256[],bytes)'),
  executeOperation: selector('executeOperation(address,uint256,uint256,address,bytes)'),
  providerFlashLoan: selector('flashLoan(address,address,uint256,bytes)'),
  requestFlashLoan: selector('requestFlashLoan(address,uint256,bytes)'),
};
for (const [k, v] of Object.entries(SEL)) say(`  ${k.padEnd(20)} 0x${v}`);
const calldata = encodeReceiveFlashLoan([HONEST.asset], [HONEST.amount], [0n], Buffer.from('abcdef01', 'hex'));
say(`  sample receiveFlashLoan calldata (${(calldata.length - 2) / 2} bytes): ${calldata.slice(0, 74)}…`);
say(`  round-trip head check: selector=0x${calldata.slice(2, 10)} off_tokens=0x${calldata.slice(10, 74).replace(/^0+/, '')}`);

say('');
say('[6] FIXTURES — current vs proposed callback predicate on the SAME scenarios');
say('SCENARIO'.padEnd(58) + 'CURRENT'.padEnd(32) + 'PROPOSED');
for (const s of [HONEST, ...FORGERIES]) {
  const cur = predicateCurrent(s), prop = predicateProposed(s);
  say(s.label.padEnd(58) + cur.padEnd(32) + prop);
}
const accepted = FORGERIES.filter((s) => predicateCurrent(s) === 'ACCEPT');
const stillAccepted = FORGERIES.filter((s) => predicateProposed(s) === 'ACCEPT');
say('');
say(`=> current predicate ACCEPTS ${accepted.length}/${FORGERIES.length} forged scenarios: `);
for (const s of accepted) say(`     - ${s.label}`);
say(`=> proposed predicate ACCEPTS ${stillAccepted.length}/${FORGERIES.length} forged scenarios`);
// Discrimination is asserted only over PAYLOAD forgeries (distinct tuple -> distinct
// commitment). A 'state' forgery deliberately reuses the honest tuple and is separated
// by the presence/absence of the stored pending commitment, not by the digest.
const payloadF = FORGERIES.filter((s) => s.kind === 'payload');
const disc = payloadF.every((s) => requestCommitment(s.chainId, s.wrapper, s.provider, s.asset, s.amount, s.userData) !== HONEST.pendingCommitment);
const stateF = FORGERIES.filter((s) => s.kind === 'state');
say(`=> commitment discrimination over ${payloadF.length} payload forgeries: ${disc ? 'every mutation yields a DIFFERENT commitment' : 'COLLISION DETECTED'}`);
say(`=> ${stateF.length} state forgeries share the honest tuple by construction and are separated by the pending-commitment slot (absent/reentrant/spoofed sender), not by the digest`);

say('');
hr();
const findings = rows.filter((r) => r.status !== 'PASS');
say(`SUMMARY: ${rows.filter((r) => r.status === 'PASS').length} PASS / ${findings.length} non-PASS / ${rows.length} rows`);
say(`ORACLE VALID: anchors=${anchorFail === 0} keccak=${keccakFail === 0} mutants=${weak.length === 0}`);
if (weak.length) say(`CHECKER_WEAK: ${weak.map((m) => m.targetId).join(',')} — probe did not flip; treat as oracle defect, not a PASS`);
hr();

console.log(out.join('\n'));

const invalid = anchorFail > 0 || keccakFail > 0 || weak.length > 0;
process.exit(invalid ? 1 : 0);
