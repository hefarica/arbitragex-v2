'use client';

/**
 * Ω-S5++ C9.∞ APEX · AllocatorClient
 * Componente client que consume el stream realtime y renderiza:
 *   - tabla (chain, strategy, α, β, yield, var, ½-Kelly, cap_usd)
 *   - estado: real / loading / blocked / unavailable / stale
 *
 * NUNCA muestra "PASS" sin evidencia. NUNCA convierte null → 0.
 */
import { useEffect, useMemo, useState } from 'react';
import {
  AllocatorSignalSchema,
  type AllocatorSignal,
  CHAIN_NAMES,
  type ChainId,
} from '@/lib/apex/schemas';
import { ApexStreamClient, decodePayload } from '@/lib/apex/realtime/streamClient';

type UiState =
  | { phase: 'connecting' }
  | { phase: 'streaming'; signals: ReadonlyArray<AllocatorSignal> }
  | { phase: 'stale'; reason: string; signals: ReadonlyArray<AllocatorSignal> }
  | { phase: 'unavailable'; reason: string };

// WS endpoint del edge: nunca apunta a internal services.
const ALLOCATOR_WS = '/edge/ws/allocator';

export default function AllocatorClient() {
  const [ui, setUi] = useState<UiState>({ phase: 'connecting' });

  useEffect(() => {
    const url = typeof window !== 'undefined'
      ? `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}${ALLOCATOR_WS}`
      : '';
    if (!url) {
      setUi({ phase: 'unavailable', reason: 'SSR cannot open WebSocket' });
      return;
    }

    const client = new ApexStreamClient({ url });
    const buffer: Map<string, AllocatorSignal> = new Map();

    const unsub = client.subscribe((ev) => {
      if (ev.kind === 'snapshot') {
        const decoded = decodePayload(ev.payload, AllocatorSignalSchema.array());
        if (!decoded) return;
        buffer.clear();
        for (const s of decoded) buffer.set(`${s.chain_id}::${s.strategy_id}`, s);
        setUi({ phase: 'streaming', signals: Array.from(buffer.values()) });
      } else if (ev.kind === 'delta') {
        const decoded = decodePayload(ev.payload, AllocatorSignalSchema);
        if (!decoded) return;
        buffer.set(`${decoded.chain_id}::${decoded.strategy_id}`, decoded);
        setUi({ phase: 'streaming', signals: Array.from(buffer.values()) });
      } else if (ev.kind === 'stale') {
        setUi((prev) => {
          if (prev.phase === 'streaming') {
            return { phase: 'stale', reason: ev.details, signals: prev.signals };
          }
          return prev;
        });
      } else if (ev.kind === 'error') {
        setUi({ phase: 'unavailable', reason: `${ev.code}: ${ev.message}` });
      }
    });

    client.connect();
    return () => {
      unsub();
      client.close();
    };
  }, []);

  const rows = useMemo(() => {
    if (ui.phase === 'streaming' || ui.phase === 'stale') {
      return [...ui.signals].sort((a, b) => a.chain_id - b.chain_id);
    }
    return [] as ReadonlyArray<AllocatorSignal>;
  }, [ui]);

  if (ui.phase === 'connecting') {
    return <div className="text-sm">Conectando al stream de scoring…</div>;
  }
  if (ui.phase === 'unavailable') {
    return (
      <div className="rounded-md border border-destructive p-4">
        <div className="font-semibold text-destructive">UNAVAILABLE</div>
        <div className="text-sm text-muted-foreground">{ui.reason}</div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {ui.phase === 'stale' && (
        <div className="rounded-md border border-amber-500/50 bg-amber-500/5 p-3 text-sm">
          <strong>STALE:</strong> {ui.reason}
        </div>
      )}
      <div className="rounded-md border overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted/30">
            <tr className="text-left">
              <th className="px-3 py-2">Chain</th>
              <th className="px-3 py-2">Strategy</th>
              <th className="px-3 py-2 text-right">α</th>
              <th className="px-3 py-2 text-right">β</th>
              <th className="px-3 py-2 text-right">yield</th>
              <th className="px-3 py-2 text-right">var</th>
              <th className="px-3 py-2 text-right">½-Kelly</th>
              <th className="px-3 py-2 text-right">cap USD</th>
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td colSpan={8} className="px-3 py-4 text-center text-muted-foreground">
                  NO_DATA — sin posteriors recibidos aún
                </td>
              </tr>
            ) : (
              rows.map((s: AllocatorSignal) => (
                <tr key={`${s.chain_id}-${s.strategy_id}`} className="border-t">
                  <td className="px-3 py-2 font-mono text-xs">
                    {CHAIN_NAMES[s.chain_id satisfies ChainId]} ({s.chain_id})
                  </td>
                  <td className="px-3 py-2 font-mono text-xs">{s.strategy_id.slice(0, 8)}…</td>
                  <td className="px-3 py-2 text-right tabular-nums">{s.posterior_alpha.toFixed(3)}</td>
                  <td className="px-3 py-2 text-right tabular-nums">{s.posterior_beta.toFixed(3)}</td>
                  <td className="px-3 py-2 text-right tabular-nums">{s.yield_ratio.toFixed(4)}</td>
                  <td className="px-3 py-2 text-right tabular-nums">{s.variance.toFixed(5)}</td>
                  <td className="px-3 py-2 text-right tabular-nums">
                    {(s.half_kelly_fraction * 100).toFixed(2)}%
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums font-semibold">
                    ${s.assigned_cap_usd}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
      <p className="text-xs text-muted-foreground">
        TTL del posterior: 300s · κ_variance_aversion = 2.0 · cap inicial Ghost = $0.00
      </p>
    </div>
  );
}
