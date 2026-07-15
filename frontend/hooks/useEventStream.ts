import { useState, useEffect } from "react";

/**
 * Minimal fail-honest SSE hook for live-testnet telemetry.
 *
 * If the server does not expose the requested SSE endpoint, the hook surfaces
 * `isConnected: false` and an empty events array instead of crashing or
 * fabricating data (R8 Fail-Honest).
 */
export function useEventStream(chainId: number) {
  const [events, setEvents] = useState<any[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!chainId) return;

    const es = new EventSource(`/api/live-testnet/events?chain_id=${chainId}`);

    es.onopen = () => {
      setIsConnected(true);
      setError(null);
    };

    es.onmessage = (e) => {
      try {
        const parsed = JSON.parse(e.data);
        setEvents((p) => [parsed, ...p].slice(0, 100));
      } catch (err) {
        setError("parse_error");
      }
    };

    es.onerror = () => {
      setIsConnected(false);
      setError("sse_unavailable");
    };

    return () => es.close();
  }, [chainId]);

  return { events, isConnected, error };
}
