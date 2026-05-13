# Patrones Correctos (Implementation)

## Patrón 1: Icon Buttons Seguros
```tsx
// 🟢 CORRECTO
import { RefreshCw } from 'lucide-react';

export function RefreshButton({ onRefresh }) {
  return (
    <button 
      onClick={onRefresh} 
      className="p-2 hover:bg-slate-800 rounded-md"
      aria-label="Refresh live feed" // Crítico para A11y y Bots
      title="Refresh"
    >
      <RefreshCw aria-hidden="true" />
    </button>
  );
}
```

## Patrón 2: Estructura Semántica del DOM
```tsx
// 🟢 CORRECTO
export function DashboardLayout({ sidebar, children }) {
  return (
    <div className="flex">
      <header className="sr-only">ArbitrageX Admin</header>
      <aside aria-label="Sidebar Navigation">{sidebar}</aside>
      <main id="main-content" className="flex-1">
        {children}
      </main>
    </div>
  );
}
```
