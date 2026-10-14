import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// D2 mirror parity (precedent: api-server prices-contract-parity.test.ts):
// the frontend mirror MUST stay byte-identical to the shared-ts source of truth.
// This test fails when either side changes without mirroring the other (drift-gate).
describe('prices-contracts mirror parity (D2)', () => {
  it('frontend/lib/prices/prices-contracts.ts === shared-ts/src/prices-contracts.ts', () => {
    const mirror = readFileSync(resolve(__dirname, 'prices-contracts.ts'), 'utf8');
    const source = readFileSync(resolve(__dirname, '../../../shared-ts/src/prices-contracts.ts'), 'utf8');
    expect(mirror).toBe(source);
  });
});