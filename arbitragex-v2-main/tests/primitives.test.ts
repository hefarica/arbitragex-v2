/**
 * Ω-S5++ C9.∞ APEX · Tests primitivos
 * Validan INV-2 hermiticity y INV-7 Ghost para wallets.
 */
import { describe, expect, it } from 'vitest';
import {
  ChainIdSchema,
  EvmAddressSchema,
  GhostBalanceSchema,
  Bytes32Schema,
  WeiAmountSchema,
  UsdDecimalSchema,
  IsoTimestampSchema,
  ReadinessStateSchema,
  BlockedResponseSchema,
} from '../frontend/lib/apex/schemas/_primitives';
import {
  WalletEntitySchema,
  CapitalGateSchema,
} from '../frontend/lib/apex/schemas/operational';

describe('ChainId', () => {
  it('accepts the 7 canonical chain ids', () => {
    for (const id of [1, 56, 137, 42161, 10, 8453, 43114]) {
      expect(ChainIdSchema.safeParse(id).success).toBe(true);
    }
  });
  it('rejects unknown chain ids', () => {
    expect(ChainIdSchema.safeParse(2).success).toBe(false);
    expect(ChainIdSchema.safeParse(0).success).toBe(false);
  });
});

describe('EvmAddress', () => {
  it('accepts 0x + 40 hex', () => {
    expect(EvmAddressSchema.safeParse('0x' + 'a'.repeat(40)).success).toBe(true);
  });
  it('rejects truncated or non-hex', () => {
    expect(EvmAddressSchema.safeParse('0x123').success).toBe(false);
    expect(EvmAddressSchema.safeParse('not-an-address').success).toBe(false);
  });
});

describe('Bytes32', () => {
  it('accepts lowercase 64 hex with 0x prefix', () => {
    expect(Bytes32Schema.safeParse('0x' + 'f'.repeat(64)).success).toBe(true);
  });
  it('rejects uppercase or wrong length', () => {
    expect(Bytes32Schema.safeParse('0x' + 'F'.repeat(64)).success).toBe(false);
    expect(Bytes32Schema.safeParse('0x' + 'f'.repeat(63)).success).toBe(false);
  });
});

describe('Money types — strings only, never numbers', () => {
  it('WeiAmount accepts decimal-string integers', () => {
    expect(WeiAmountSchema.safeParse('1000000000000000000').success).toBe(true);
    expect(WeiAmountSchema.safeParse('0').success).toBe(true);
  });
  it('WeiAmount rejects numeric and float strings', () => {
    expect(WeiAmountSchema.safeParse(1).success).toBe(false);
    expect(WeiAmountSchema.safeParse('1.5').success).toBe(false);
    expect(WeiAmountSchema.safeParse('-1').success).toBe(false);
  });
  it('UsdDecimal accepts up to 6 decimals as string', () => {
    expect(UsdDecimalSchema.safeParse('0').success).toBe(true);
    expect(UsdDecimalSchema.safeParse('0.00').success).toBe(true);
    expect(UsdDecimalSchema.safeParse('1234.567890').success).toBe(true);
  });
  it('UsdDecimal rejects scientific notation', () => {
    expect(UsdDecimalSchema.safeParse('1e6').success).toBe(false);
  });
});

describe('IsoTimestamp requires offset', () => {
  it('accepts ISO with offset', () => {
    expect(IsoTimestampSchema.safeParse('2026-05-14T18:00:00+00:00').success).toBe(true);
    expect(IsoTimestampSchema.safeParse('2026-05-14T18:00:00Z').success).toBe(true);
  });
  it('rejects timestamp without TZ', () => {
    expect(IsoTimestampSchema.safeParse('2026-05-14T18:00:00').success).toBe(false);
  });
});

describe('GhostBalance literal', () => {
  it('accepts only "0"', () => {
    expect(GhostBalanceSchema.safeParse('0').success).toBe(true);
    expect(GhostBalanceSchema.safeParse('1').success).toBe(false);
    expect(GhostBalanceSchema.safeParse(0).success).toBe(false);
  });
});

describe('ReadinessState enum', () => {
  it('accepts canonical states', () => {
    for (const s of ['PASS', 'PARTIAL', 'BLOCKED', 'UNAVAILABLE', 'NO_DATA', 'PENDING_RUNTIME_ACK']) {
      expect(ReadinessStateSchema.safeParse(s).success).toBe(true);
    }
  });
  it('rejects success/FAIL aliases', () => {
    expect(ReadinessStateSchema.safeParse('SUCCESS').success).toBe(false);
    expect(ReadinessStateSchema.safeParse('FAIL').success).toBe(false);
  });
});

describe('BlockedResponse honesty', () => {
  it('requires reason + next_action', () => {
    const v = {
      status: 'BLOCKED',
      reason: 'rpc_http_1 unhealthy',
      next_action: 'rotate provider',
      unblock_requirements: ['rpc_p95<200ms'],
      blocker_codes: ['RPC_HTTP_1'],
      evaluated_at: '2026-05-14T18:00:00Z',
    };
    expect(BlockedResponseSchema.safeParse(v).success).toBe(true);
  });
  it('rejects empty reason', () => {
    const v = {
      status: 'BLOCKED', reason: '', next_action: 'x',
      unblock_requirements: [], blocker_codes: [],
      evaluated_at: '2026-05-14T18:00:00Z',
    };
    expect(BlockedResponseSchema.safeParse(v).success).toBe(false);
  });
});

describe('WalletEntity Ghost invariant (INV-7)', () => {
  const base = {
    wallet_id: '11111111-1111-4111-8111-111111111111',
    chain_id: 1 as const,
    address: '0x' + 'a'.repeat(40),
    enabled: true,
    rotated_at: null,
    readiness: 'PASS' as const,
    config_hash: '0x' + 'f'.repeat(64),
    updated_at: '2026-05-14T18:00:00Z',
    sequence_id: 1,
  };
  it('EXECUTION_SIGNER with balance=0 passes', () => {
    expect(
      WalletEntitySchema.safeParse({ ...base, kind: 'EXECUTION_SIGNER', balance_wei: '0' }).success,
    ).toBe(true);
  });
  it('EXECUTION_SIGNER with balance>0 fails', () => {
    expect(
      WalletEntitySchema.safeParse({ ...base, kind: 'EXECUTION_SIGNER', balance_wei: '1' }).success,
    ).toBe(false);
  });
  it('CAPITAL_PROVIDER can hold balance', () => {
    expect(
      WalletEntitySchema.safeParse({ ...base, kind: 'CAPITAL_PROVIDER', balance_wei: '1000000' }).success,
    ).toBe(true);
  });
});

describe('CapitalGate sovereign signature requirement (INV-A)', () => {
  const base = {
    gate_id: '22222222-2222-4222-8222-222222222222',
    chain_id: 1 as const,
    current_exposure_usd: '0.00',
    enabled: true,
    config_hash: '0x' + 'f'.repeat(64),
    updated_at: '2026-05-14T18:00:00Z',
    sequence_id: 1,
  };
  it('cap_usd=0 with null signature passes', () => {
    expect(
      CapitalGateSchema.safeParse({
        ...base, cap_usd: '0.00',
        sovereign_signature: null, signature_payload_hash: null,
      }).success,
    ).toBe(true);
  });
  it('cap_usd>0 without signature fails', () => {
    expect(
      CapitalGateSchema.safeParse({
        ...base, cap_usd: '100.00',
        sovereign_signature: null, signature_payload_hash: null,
      }).success,
    ).toBe(false);
  });
  it('cap_usd>0 with signature passes', () => {
    expect(
      CapitalGateSchema.safeParse({
        ...base, cap_usd: '100.00',
        sovereign_signature: '0x' + 'a'.repeat(130),
        signature_payload_hash: '0x' + 'b'.repeat(64),
      }).success,
    ).toBe(true);
  });
});
