# Patrones Correctos (Implementation)

## Patrón 1: URL as State for Filters (App Router)
```tsx
'use client';
import { useRouter, usePathname, useSearchParams } from 'next/navigation';

export function SearchFilter() {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  const handleSearch = (term: string) => {
    const params = new URLSearchParams(searchParams.toString());
    if (term) {
      params.set('q', term);
    } else {
      params.delete('q');
    }
    // Preserva estado nativo y compartible
    router.replace(`${pathname}?${params.toString()}`);
  }

  return <input onChange={(e) => handleSearch(e.target.value)} defaultValue={searchParams.get('q') || ''} />;
}
```

## Patrón 2: Zustand para Preferencias de UI / Tokens Live
```ts
import { create } from 'zustand';

interface UIStore {
  compactMode: boolean;
  toggleCompact: () => void;
}

export const useUIStore = create<UIStore>((set) => ({
  compactMode: false,
  toggleCompact: () => set((state) => ({ compactMode: !state.compactMode })),
}));
```
