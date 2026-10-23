#!/usr/bin/env node
// =============================================================================
// CONTRACTS-FIXCHECK-01 — does the generated fix actually close the probes?
// =============================================================================
// Re-applies the SAME predicates as CONTRACTS-ORACLE-01 rows C4 / C4c / C7 / C10 /
// C11 / C12 against docs/contracts/proposed/FlashLoanExecutor.fixed.sol and reports
// whether each verdict flips relative to the anchored original.
//
// HONESTY BOUNDARY: this is a REGEX CROSS-CHECK, not a compiler and not a test run.
// It shows the fix introduces the constructs the oracle looked for and removes the
// construct it flagged. It does NOT prove the contract compiles, that the storage
// layout stays valid, or that the callbacks behave under fuzzing. Those require
// forge(1) on a host where Windows AppControl does not block it (os error 4551).
// =============================================================================

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const REPO = process.argv.includes('--repo') ? process.argv[process.argv.indexOf('--repo') + 1] : '.';
const read = (p) => readFileSync(join(REPO, p), 'utf8');

function strip(s) {
  let res = '', i = 0;
  while (i < s.length) {
    if (s[i] === '/' && s[i + 1] === '/') { while (i < s.length && s[i] !== '\n') { res += ' '; i++; } continue; }
    if (s[i] === '/' && s[i + 1] === '*') { i += 2; while (i < s.length && !(s[i] === '*' && s[i + 1] === '/')) { res += (s[i] === '\n' ? '\n' : ' '); i++; } i += 2; continue; }
    if (s[i] === '"' || s[i] === "'") { const q = s[i]; res += ' '; i++; while (i < s.length && s[i] !== q) { res += (s[i] === '\\' ? ' ' : (s[i] === '\n' ? '\n' : ' ')); if (s[i] === '\\') i++; i++; } res += ' '; i++; continue; }
    res += s[i]; i++;
  }
  return res;
}
function fnBody(code, name) {
  const re = new RegExp(`function\\s+${name}\\s*\\(`, 'g');
  const m = re.exec(code);
  if (!m) return null;
  const open = code.indexOf('{', m.index);
  let d = 0, i = open;
  for (; i < code.length; i++) { if (code[i] === '{') d++; else if (code[i] === '}') { d--; if (d === 0) break; } }
  return code.slice(open, i + 1);
}

const before = strip(read('docs/contracts/anchored/FlashLoanExecutor.sol'));
const after = strip(read('docs/contracts/proposed/FlashLoanExecutor.fixed.sol'));
const SESSION_TOKENS = ['_pending', 'pendingRequest', 'session', 'commitment', 'requestHash', '_inflight', '_activeRequest', 'tstore', 'transient', 'initiator'];

const checks = [
  {
    id: 'C4', label: 'FL-3 session/request binding (Balancer callback)',
    before: () => SESSION_TOKENS.some((t) => (fnBody(before, 'receiveFlashLoan') || '').includes(t)) ? 'PASS' : 'ABSENT',
    after: () => SESSION_TOKENS.some((t) => (fnBody(after, 'receiveFlashLoan') || '').includes(t)) ? 'PASS' : 'ABSENT',
  },
  {
    id: 'C4c', label: 'F25 producer side records a request identity',
    before: () => /_pending\w*\s*=/.test(fnBody(before, 'requestFlashLoan') || '') ? 'PASS' : 'ABSENT',
    after: () => /_pending\w*\s*=/.test(fnBody(after, 'requestFlashLoan') || '') ? 'PASS' : 'ABSENT',
  },
  {
    id: 'C7', label: 'FL-5 on-chain profit floor before repay',
    before: () => ['minProfitFloor', 'profitFloor'].some((t) => (fnBody(before, 'receiveFlashLoan') || '').includes(t)) ? 'PASS' : 'ABSENT',
    after: () => ['minProfitFloor', 'profitFloor'].some((t) => (fnBody(after, 'receiveFlashLoan') || '').includes(t)) ? 'PASS' : 'ABSENT',
  },
  {
    id: 'C10', label: 'F26 repayment scoped to operation delta (not total balance)',
    // FAIL when the gate compares the TOTAL balance; PASS when a delta baseline exists.
    before: () => /balanceOf\(address\(this\)\)\s*<\s*amountOwed/.test(after === undefined ? '' : fnBody(before, 'receiveFlashLoan') || '') ? 'FAIL' : 'PASS',
    after: () => {
      const b = fnBody(after, 'receiveFlashLoan') || '';
      const totalBalanceGate = /balanceOf\(address\(this\)\)\s*<\s*amountOwed\s*\)/.test(b);
      const delta = /balBeforeCallback\s*\+/.test(b);
      return (!totalBalanceGate && delta) ? 'PASS' : 'FAIL';
    },
  },
  {
    id: 'C11', label: 'F27 ERC-20 profit exit present',
    before: () => /function\s+\w*(sweep|withdraw|rescue)\w*\s*\(/i.test(before) ? 'PASS' : 'ABSENT',
    after: () => /function\s+\w*(sweep|withdraw|rescue)\w*\s*\(/i.test(after) ? 'PASS' : 'ABSENT',
  },
  {
    id: 'C12', label: 'F28 wrapper pausable with a scoped guardian',
    before: () => /Pausable|whenNotPaused/.test(before) ? 'PASS' : 'ABSENT',
    after: () => {
      const pausable = /PausableUpgradeable/.test(after);
      const guardian = /GUARDIAN_ROLE/.test(after);
      const pauseGuard = /function\s+pause\(\)\s+external\s+onlyRole\(GUARDIAN_ROLE\)/.test(after);
      const unpauseAdmin = /function\s+unpause\(\)\s+external\s+onlyRole\(DEFAULT_ADMIN_ROLE\)/.test(after);
      return (pausable && guardian && pauseGuard && unpauseAdmin) ? 'PASS' : 'ABSENT';
    },
  },
];

let fixed = 0;
console.log('CONTRACTS-FIXCHECK-01 — regex cross-check of the generated fix');
console.log('(NOT a compile and NOT a test run; see the honesty boundary in the header)\n');
console.log('ID    RULE'.padEnd(58) + 'BEFORE'.padEnd(10) + 'AFTER'.padEnd(10) + 'CLOSED');
for (const c of checks) {
  const b = c.before(), a = c.after();
  const closed = b !== 'PASS' && a === 'PASS';
  if (closed) fixed++;
  console.log(c.id.padEnd(6) + c.label.padEnd(52) + b.padEnd(10) + a.padEnd(10) + (closed ? 'yes' : (b === a ? 'same' : 'CHANGED?')));
}
console.log(`\n=> ${fixed}/${checks.length} flagged probes flip to PASS on the generated file`);
console.log(`REMAINING FRONTIER: compile + unit + fuzz + invariant + fork, and the storage-layout pin test`);
console.log(`(test/StorageLayout.t.sol) — all require forge(1), blocked on this host by Windows AppControl.`);
