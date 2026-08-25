/**
 * Ω ARBX-DP-005 · Execution_Class → tier fold, DIFFERENTIAL against the real
 * generated catalog.
 *
 * The TS mirror (`lib/apex/signal-tier.ts`) and the Rust rule
 * (`backend/searcher-rs/src/signal_tier.rs`) must agree on the SAME table:
 * the Rust twin sweeps `DETECTOR_POLICIES` and pins [1, 15, 33, 11]; this
 * file sweeps the REAL generated wire table (QUOTEBASE_DETECTOR_CATALOG —
 * the EMIT-08 source) and pins the same partition. If a workbook ingestion
 * changes any class, BOTH alarms fire together and the FE panel's four
 * feeds re-derive from the new canon (no hardcoded counts in the schema,
 * §28 — the numbers live ONLY in these differential tests).
 */
import { describe, expect, it } from 'vitest';

import {
  QUOTEBASE_DETECTOR_CATALOG,
  QUOTEBASE_STRATEGY_CATALOG,
} from '../../../../backend/api-server/src/generated/quotebase_catalog';

import { SIGNAL_TIER_TOKENS, tierForExecutionClass } from '../signal-tier';

function tierCounts<T extends { execution_class: string }>(rows: readonly T[]) {
  const counts = { observation: 0, signal: 0, candidate: 0, executable: 0, unknown: 0 };
  for (const row of rows) {
    const tier = tierForExecutionClass(row.execution_class);
    if (tier === null) counts.unknown += 1;
    else counts[tier] += 1;
  }
  return counts;
}

describe('ARBX-DP-005 · tierForExecutionClass — differential sweep (60 detectors)', () => {
  it('pins the four-way partition 1 / 15 / 33 / 11 with ZERO unknown (workbook-version invariant)', () => {
    expect(tierCounts(QUOTEBASE_DETECTOR_CATALOG)).toEqual({
      observation: 1,
      signal: 15,
      candidate: 33,
      executable: 11,
      unknown: 0,
    });
  });

  it('OBSERVATION ⟺ OBSERVE_ONLY (exactly 1, and it is the only observation row)', () => {
    const obs = QUOTEBASE_DETECTOR_CATALOG.filter((d) => d.execution_class === 'OBSERVE_ONLY');
    expect(obs).toHaveLength(1);
    expect(tierForExecutionClass(obs[0]!.execution_class)).toBe('observation');
  });

  it('EXECUTABLE ⟺ DETERMINISTIC_EXECUTABLE — no other class reaches the executable tier', () => {
    const det = QUOTEBASE_DETECTOR_CATALOG.filter(
      (d) => d.execution_class === 'DETERMINISTIC_EXECUTABLE',
    );
    expect(det).toHaveLength(11);
    for (const row of QUOTEBASE_DETECTOR_CATALOG) {
      const tier = tierForExecutionClass(row.execution_class);
      if (tier === 'executable') {
        expect(row.execution_class).toBe('DETERMINISTIC_EXECUTABLE');
      }
    }
  });

  it('DETERMINISTIC_* folds candidate unless it is exactly _EXECUTABLE (rule shape)', () => {
    for (const row of QUOTEBASE_DETECTOR_CATALOG) {
      if (!row.execution_class.startsWith('DETERMINISTIC_')) continue;
      const tier = tierForExecutionClass(row.execution_class);
      if (row.execution_class === 'DETERMINISTIC_EXECUTABLE') {
        expect(tier).toBe('executable');
      } else {
        expect(tier).toBe('candidate');
      }
    }
  });

  it('every class the catalog carries is INSIDE the closed vocabulary (drift alarm)', () => {
    const distinct = new Set(QUOTEBASE_DETECTOR_CATALOG.map((d) => d.execution_class));
    for (const cls of distinct) {
      expect(
        tierForExecutionClass(cls),
        `execution_class "${cls}" drifted outside the closed vocabulary`,
      ).not.toBeNull();
    }
  });

  it('strategy side: all 264 rows fold with ZERO unknown; 8/92/127/37 (workbook-version)', () => {
    const counts = tierCounts(QUOTEBASE_STRATEGY_CATALOG);
    expect(counts).toEqual({
      observation: 8,
      signal: 92,
      candidate: 127,
      executable: 37,
      unknown: 0,
    });
  });
});

describe('ARBX-DP-005 · fail-closed vocabulary (R8/§28)', () => {
  it('tokens outside the closed 29 fold to null — never a default tier', () => {
    expect(tierForExecutionClass('PROBABLY_GOOD')).toBeNull();
    expect(tierForExecutionClass('')).toBeNull();
    expect(tierForExecutionClass('DETERMINISTIC')).toBeNull(); // prefix, not the token
    expect(tierForExecutionClass('observe_only')).toBeNull(); // case-sensitive vocabulary
    expect(tierForExecutionClass('OBSERVE_ONLY ')).toBeNull(); // trailing space = drift
    expect(tierForExecutionClass('DETERMINISTIC_EXECUTABLE_TOO')).toBeNull();
  });

  it('SIGNAL_TIER_TOKENS are the four stable as_str tokens (Rust parity)', () => {
    expect(SIGNAL_TIER_TOKENS).toEqual(['observation', 'signal', 'candidate', 'executable']);
  });
});
