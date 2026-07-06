# SSOT Architecture Patterns Reference

This document outlines the core patterns for implementing a Single Source of Truth (SSOT) architecture in React/Next.js frontends, specifically following the "Eficiencia Absoluta" doctrine.

## Core Doctrine: Eficiencia Absoluta (Absolute Efficiency)

1. **Nothing is discarded**: Existing presentation code is reused and wired to the SSOT.
2. **Everything is threaded**: Pages consume data via centralized Zustand selectors.
3. **Zero redundant fetches**: Dynamic arrays (`.map`, `.filter`) and conditional rendering replace per-page API calls.
4. **Fail-honest**: The UI renders exactly what the API/SSOT provides. Empty array = empty state. No mocks.

## 1. The Mounted Snapshot Pattern (SSR + Client)

Use this pattern for Next.js App Router pages that need initial server data but then hydrate from the SSOT.

```tsx
// page.tsx (Server Component)
export default async function Page() {
  const initialSnapshot = await fetchEdgeData();
  return <ClientComponent initialSnapshot={initialSnapshot} />;
}

// ClientComponent.tsx (Client Component)
'use client';
export function ClientComponent({ initialSnapshot }) {
  // Hydrate store on mount if needed, or just use local state
  // Everything non-deterministic (Date.now(), window) MUST be inside useEffect
  const [data, setData] = useState(initialSnapshot);
  
  // ...
}
```

## 2. Dynamic Selector Pattern (Zustand)

Avoid fetching data that already exists in the global state. Use memoized selectors to derive data.

```typescript
// ✅ CORRECT: Derived state from SSOT
const useDexesByChain = (chainId: number) => {
  return useSystemStore((s) =>
    s.dexes.filter((d) => d.chain_id === chainId && d.status === 'active')
  );
};

// ❌ INCORRECT: Redundant fetch
const [dexes, setDexes] = useState([]);
useEffect(() => {
  fetch(`/api/dexes?chain_id=${chainId}`).then(setDexes);
}, [chainId]);
```

## 3. High-Volume Data Rendering (5000+ items)

When rendering thousands of items (like liquidity pools or tokens) from the SSOT, prevent memory saturation and UI freezing.

```tsx
import { VirtualizedList } from '@/components/ui';

const PoolsTab = () => {
  const selectedDexId = useSystemStore((s) => s.selectedDexId);
  
  // Memoized selector: only recalculates if dexId or pools change
  const pools = useMemo(
    () => useSystemStore.getState().pools.filter((p) => p.dex_id === selectedDexId),
    [selectedDexId]
  );

  // Render only visible items using virtualization
  return (
    <VirtualizedList
      items={pools}
      renderItem={(pool) => <PoolRow pool={pool} />}
      itemHeight={60}
      overscan={5}
    />
  );
};
```

## 4. Cascading Selectors without Refetch

When one selection depends on another (e.g., Chain -> DEX -> Pool), derive all options from the SSOT based on the current selection state.

```tsx
const { activeChainId, selectedDexId } = useSystemStore((s) => ({
  activeChainId: s.activeChainId,
  selectedDexId: s.selectedDexId,
}));

// Selector 1: DEXes for active chain
const dexes = useMemo(() => 
  useSystemStore.getState().dexes.filter(d => d.chain_id === activeChainId),
  [activeChainId]
);

// Selector 2: Pools for selected DEX
const pools = useMemo(() => 
  useSystemStore.getState().pools.filter(p => p.dex_id === selectedDexId),
  [selectedDexId]
);

// Handle selection change by updating SSOT and clearing dependent state
const handleDexChange = (dexId) => {
  useSystemStore.setState({ 
    selectedDexId: dexId,
    selectedPoolId: '' // Clear dependent selection
  });
};
```

## 5. Dynamic WebSocket Injection

Connect to real-time streams based on the currently selected environment/chain in the SSOT.

```typescript
export const useOpportunitiesStream = () => {
  const { activeChainId, wsUrl } = useSystemStore((s) => ({
    activeChainId: s.activeChainId,
    wsUrl: s.chains.find((c) => c.chain_id === s.activeChainId)?.rpc_ws,
  }));

  useEffect(() => {
    if (!wsUrl) return; // Fail-honest: no URL, no connection

    const ws = new WebSocket(wsUrl);
    
    ws.onopen = () => {
      // Subscribe based on SSOT state
      ws.send(JSON.stringify({
        method: 'subscribe',
        params: ['opportunities', { chain_id: activeChainId }]
      }));
    };

    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      // Update SSOT with new data
      useSystemStore.setState((s) => ({
        opportunities: [data.params, ...s.opportunities].slice(0, 1000)
      }));
    };

    return () => ws.close();
  }, [wsUrl, activeChainId]); // Reconnect if URL or chain changes

  // Return the live data from SSOT
  return useSystemStore((s) => s.opportunities);
};
```
