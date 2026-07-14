import { useState, useEffect } from "react";

export function useEventStream(chainId: number) {
  const [events, setEvents] = useState<any[]>([]);
  const [isConnected, setIsConnected] = useState(false);

  useEffect(() => {
    const es = new EventSource(`/api/live-testnet/events?chain_id=${chainId}`);
    es.onopen = () => setIsConnected(true);
    es.onmessage = (e) => setEvents((p) => [JSON.parse(e.data), ...p].slice(0, 100));
    es.onerror = () => setIsConnected(false);
    return () => es.close();
  }, [chainId]);

  return { events, isConnected };
}
