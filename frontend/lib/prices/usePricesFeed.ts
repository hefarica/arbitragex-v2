'use client';

import { useEffect, useRef, useState } from 'react';
import { createOpportunitySocket } from '../../features/opportunities/socket-lifecycle';
import { createSeqGuard, isPriceTick } from './prices-contracts';

const SILENCE_MS = 45000;
const CHECK_MS = 5000;

/**
 * v4: NO crea socket propio. Usa la MISMA factory que el feed de oportunidades,
 * anadiendo el callback onPriceTick (patron onStreamEvent del repo).
 */
export function usePricesFeed(opts: { url: string; ioFactory: Parameters<typeof createOpportunitySocket>[0]['ioFactory']; authToken?: string; onTick?: (t: unknown) => void }) {
  const [stale, setStale] = useState(false);
  const lastMsg = useRef<number>(Date.now());
  const accept = useRef(createSeqGuard()).current;
  const handle = useRef<{ dispose: () => void } | null>(null);
  const onTickRef = useRef(opts.onTick);
  onTickRef.current = opts.onTick;

  useEffect(() => {
    const h = createOpportunitySocket({
      url: opts.url,
      ioFactory: opts.ioFactory,
      authToken: opts.authToken,
      onStatus: (s) => setStale(s === 'STALE'),
      onOpportunity: () => {},
      onPriceTick: (raw) => {
        if (!isPriceTick(raw)) return;
        if (!accept(raw)) return; // replay fuera de orden
        lastMsg.current = Date.now();
        setStale(false);
        onTickRef.current?.(raw);
      },
    });
    handle.current = h;

    const timer = setInterval(() => {
      if (document.visibilityState !== 'visible') return;
      if (Date.now() - lastMsg.current <= SILENCE_MS) return;
      setStale(true);
      // Reemitir en vez de reconectar: no resetea el contador interno de 5.
      // El socket se re-suscribe en el handler 'connect' de la factory.
    }, CHECK_MS);

    function onVisible() {
      if (document.visibilityState !== 'visible') return;
      lastMsg.current = Date.now();
    }
    document.addEventListener('visibilitychange', onVisible);

    return () => {
      clearInterval(timer);
      document.removeEventListener('visibilitychange', onVisible);
      h.dispose();
      handle.current = null;
    };
  }, [opts.url, opts.authToken, opts.ioFactory, accept]);

  return { stale };
}
