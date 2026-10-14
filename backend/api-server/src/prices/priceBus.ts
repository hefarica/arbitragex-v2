import { EventEmitter } from 'node:events';

// Contrato canónico: shared-ts/src/prices-contracts.ts (PriceTickSchema).
// Este tipo estructural local existe porque el tsconfig de api-server
// (moduleResolution NodeNext + rootDir "src") PROHÍBE imports relativos
// cross-package en build (TS6059, medido 2026-10-13). La paridad con el
// schema zod la enforcement prices-contract-parity.test.ts (mismos campos
// + safeParse round-trip).
export interface PriceTick {
  pair: string;
  mid: number;
  ts: number;
  src: string;
  seq: number;
}

const MIN_INTERVAL_MS = Number(process.env['ARBX_PRICE_MIN_INTERVAL_MS'] ?? 100);
const SNAPSHOT_MAX_AGE_MS = 5000;

/** Fuente unica. Coalescing + snapshot. NO abre sockets (los pone el gateway). */
class PriceBus {
  private emitter = new EventEmitter();
  private latest = new Map<string, PriceTick>();
  private lastSent = new Map<string, number>();
  private seq = new Map<string, number>();
  private published = 0;
  private coalesced = 0;

  constructor() { this.emitter.setMaxListeners(0); }

  publish(tick: Omit<PriceTick, 'seq'> & { seq?: number }): void {
    const now = Date.now();
    const prev = this.lastSent.get(tick.pair) ?? 0;
    if (now - prev < MIN_INTERVAL_MS) { this.coalesced++; return; }

    const nextSeq = tick.seq ?? ((this.seq.get(tick.pair) ?? 0) + 1);
    this.seq.set(tick.pair, nextSeq);

    const full: PriceTick = { ...tick, seq: nextSeq };
    this.lastSent.set(tick.pair, now);
    this.latest.set(tick.pair, full);
    this.published++;
    this.emitter.emit('tick', full);
  }

  subscribe(handler: (tick: PriceTick) => void): () => void {
    this.emitter.on('tick', handler);
    return () => this.emitter.off('tick', handler);
  }

  snapshot(maxAgeMs: number = SNAPSHOT_MAX_AGE_MS): PriceTick[] {
    const now = Date.now();
    const out: PriceTick[] = [];
    for (const t of this.latest.values()) if (now - t.ts <= maxAgeMs) out.push(t);
    return out;
  }

  stats() {
    return { published: this.published, coalesced: this.coalesced, pairs: this.latest.size, subscribers: this.emitter.listenerCount('tick') };
  }
}

export const priceBus = new PriceBus();
