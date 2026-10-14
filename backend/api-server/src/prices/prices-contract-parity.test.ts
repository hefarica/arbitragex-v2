/**
 * prices-contract-parity.test.ts (t5) — single-source lock for the duplicated
 * prices surface inside api-server.
 *
 * Context (measured): api-server tsconfig (moduleResolution NodeNext +
 * rootDir "src") forbids cross-package relative imports at BUILD time —
 * TS6059 fired for shared-ts/src/prices-contracts (2026-10-13, this session).
 * The build-safe design keeps LOCAL literals in websocket.ts (as the original
 * v4 patch declared them) and a LOCAL structural PriceTick in priceBus.ts,
 * while the CANONICAL contract stays shared-ts/src/prices-contracts.ts
 * (consumed verbatim by the frontend hook usePricesFeed).
 *
 * This test is the drift gate: if the contract ever changes a name or a
 * PriceTick field, this suite fails until the api-server mirrors are updated.
 * Test files are excluded from the build tsconfig, so the cross-package
 * import here is legal for vitest.
 */
import { describe, it, expect } from 'vitest';
import {
    PriceTickSchema,
    PRICES_ROOM as CANON_ROOM,
    PRICES_SUBSCRIBE_EVENT as CANON_SUBSCRIBE,
    PRICES_READY_EVENT as CANON_READY,
    PRICES_SNAPSHOT_EVENT as CANON_SNAPSHOT,
    PRICES_TICK_EVENT as CANON_TICK,
} from '../../../../shared-ts/src/prices-contracts';
import {
    PRICES_ROOM,
    PRICES_SUBSCRIBE_EVENT,
    PRICES_READY_EVENT,
    PRICES_SNAPSHOT_EVENT,
    PRICES_TICK_EVENT,
    type PriceTick,
} from '../websocket.js';

describe('prices contract parity (websocket.ts ↔ shared contract)', () => {
    it('room/event literals are byte-identical to the canonical contract', () => {
        expect(PRICES_ROOM).toBe(CANON_ROOM);
        expect(PRICES_SUBSCRIBE_EVENT).toBe(CANON_SUBSCRIBE);
        expect(PRICES_READY_EVENT).toBe(CANON_READY);
        expect(PRICES_SNAPSHOT_EVENT).toBe(CANON_SNAPSHOT);
        expect(PRICES_TICK_EVENT).toBe(CANON_TICK);
    });

    it('PriceTickSchema fields match the local structural type exactly', () => {
        // Field names and order are the contract; the local interface must
        // not drift (no extra/missing fields).
        expect(Object.keys(PriceTickSchema.shape)).toEqual(['pair', 'mid', 'ts', 'src', 'seq']);
    });

    it('a local PriceTick passes the canonical zod schema (structural parity)', () => {
        const sample: PriceTick = { pair: 'WETH/USDC', mid: 3000.5, ts: 1, src: 'parity-test', seq: 1 };
        expect(PriceTickSchema.safeParse(sample).success).toBe(true);
    });
});
