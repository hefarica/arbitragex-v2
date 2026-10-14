import { z } from 'zod';

// Convencion del repo: zod + imports con extension .js.
export const PriceTickSchema = z.object({
  pair: z.string().min(1),
  mid: z.number().finite(),
  ts: z.number().int().nonnegative(),
  src: z.string().min(1),
  seq: z.number().int().nonnegative(),
});
export type PriceTick = z.infer<typeof PriceTickSchema>;

export const PricesSnapshotSchema = z.object({ ticks: z.array(PriceTickSchema) });

// Nombres alineados al patron subscribe:* del gateway y al comentario
// G-PRICE-1 de /api/prices/live ("WS prices:snapshot equivalent").
export const PRICES_ROOM = 'prices';
export const PRICES_SUBSCRIBE_EVENT = 'subscribe:prices';
export const PRICES_READY_EVENT = 'prices:ready';
export const PRICES_SNAPSHOT_EVENT = 'prices:snapshot';
export const PRICES_TICK_EVENT = 'prices:tick';

export function isPriceTick(v: unknown): v is PriceTick {
  return PriceTickSchema.safeParse(v).success;
}

/** Guarda de orden/idempotencia (STREAM-SEQ-01: seq monotono por par). */
export function createSeqGuard() {
  const last = new Map<string, number>();
  return (tick: PriceTick): boolean => {
    const prev = last.get(tick.pair) ?? -1;
    if (tick.seq <= prev) return false;
    last.set(tick.pair, tick.seq);
    return true;
  };
}
