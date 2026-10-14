import { describe, it, expect } from 'vitest';
import { priceBus } from './priceBus';

describe('priceBus (contrato de precios)', () => {
  it('asigna seq monotono por par', () => {
    const seen: number[] = [];
    const off = priceBus.subscribe((t) => { if (t.pair === 'A/T') seen.push(t.seq); });
    priceBus.publish({ pair: 'A/T', mid: 1, ts: Date.now(), src: 't' });
    const t0 = Date.now(); while (Date.now() - t0 < 3) {}
    priceBus.publish({ pair: 'A/T', mid: 2, ts: Date.now(), src: 't' });
    off();
    for (let i = 1; i < seen.length; i++) expect(seen[i]).toBeGreaterThan(seen[i - 1]);
  });

  it('coalescing descarta por encima del limite', () => {
    const before = priceBus.stats().coalesced;
    for (let i = 0; i < 50; i++) priceBus.publish({ pair: 'B/T', mid: i, ts: Date.now(), src: 'b' });
    expect(priceBus.stats().coalesced).toBeGreaterThan(before);
  });

  it('snapshot solo devuelve lo fresco', () => {
    // t5: determinizado — el original v4 usaba snapshot(0) sobre un tick con
    // ts=Date.now() del MISMO milisegundo (0 <= 0 ⇒ flaky: pasó en run1,
    // falló en run2 del mismo árbol). Semántica del bus (<=) intacta; el test
    // ahora fija edades explícitas: viejo (6000ms) excluido, fresco incluido.
    const now = Date.now();
    priceBus.publish({ pair: 'S/T', mid: 9, ts: now - 6000, src: 's' });
    priceBus.publish({ pair: 'S/U', mid: 8, ts: now, src: 's' });
    expect(priceBus.snapshot(5000).some((t) => t.pair === 'S/T')).toBe(false);
    expect(priceBus.snapshot(5000).some((t) => t.pair === 'S/U')).toBe(true);
  });

  it('anti-fuga: N suscripciones y bajas no alteran el conteo base', () => {
    const base = priceBus.stats().subscribers;
    const offs = Array.from({ length: 25 }, () => priceBus.subscribe(() => {}));
    expect(priceBus.stats().subscribers).toBe(base + 25);
    for (const off of offs) off();
    expect(priceBus.stats().subscribers).toBe(base);
  });
});
