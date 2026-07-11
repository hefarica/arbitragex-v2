/**
 * Ω-S5++ C9.∞ APEX · /apex/allocator
 * -------------------------------------------------------------------------
 * Espejo en tiempo real del bayesian_allocator.rs (cierre v5).
 *
 * Consume:  arbx:scoring:updates:<chain>  (vía WebSocket)
 * Muestra:  α, β posteriors, yield_ratio, varianza, ½-Kelly, cap asignado
 * Ghost:    cap_usd inicia en "0.00"  (INV-7 + cap inicial $0)
 *
 * MIRROR LAW: este archivo vive en /apex/* — extiende, no reemplaza, las
 * 37 rutas existentes. Los estilos heredan tailwind base sin modificarlo.
 */
import { Suspense } from 'react';
import AllocatorClient from './AllocatorClient';

export const dynamic = 'force-dynamic';

export default function AllocatorPage() {
  return (
    <main className="container mx-auto px-4 py-6">
      <header className="mb-6">
        <h1 className="text-2xl font-semibold tracking-tight">
          Bayesian Allocator · Posterior espejo
        </h1>
        <p className="text-sm text-muted-foreground mt-1">
          Stream live de <code className="font-mono text-xs">arbx:scoring:updates:&lt;chain&gt;</code> · Ghost
          Protocol: cap inicial $0.00 USD · Escalación requiere firma sovereign.
        </p>
      </header>
      <Suspense fallback={<div className="text-sm">Cargando snapshot…</div>}>
        <AllocatorClient />
      </Suspense>
    </main>
  );
}
