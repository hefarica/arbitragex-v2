# Antipatrones Prohibidos

## Antipatrón 1: Sincronía en el Monolito Client-Side
```tsx
// 🔴 PROHIBIDO: Esto arrastrará toda la librería 'recharts' o 'three.js' al bundle principal
'use client';

import { 3DModelViewer } from 'heavy-3d-lib';
import { LineChart, Tooltip, XAxis } from 'recharts';

export function Dashboard() {
  // ...
}
```

## Antipatrón 2: Layout Shift por Falta de Dimensiones
```tsx
// 🔴 PROHIBIDO: Provoca CLS (Salto del layout) al cargar
<img src="/logo.png" />

// 🟢 CORRECTO
import Image from 'next/image';
<Image src="/logo.png" width={200} height={50} alt="Logo" priority />
```
