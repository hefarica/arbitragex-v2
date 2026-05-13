# Antipatrones Prohibidos

## Antipatrón 1: Type Assertion Ciego
El infame `as Type` que apaga el detector de mentiras del compilador.

```ts
// 🔴 PROHIBIDO
const res = await fetch('/api/data');
const json = await res.json() as MiEntidadFuerte; // TS confía a ciegas, si el JSON es diferente, explota en Runtime en la UI.
```

## Antipatrón 2: Interfaces Gigantes con todo Opcional
```ts
// 🔴 PROHIBIDO
interface WSPayload {
  type: string; // Puede ser cualquier cosa
  data?: Opportunity;
  errorMsg?: string;
  heartbeatTime?: number;
}
// Esto obliga al consumidor a revisar qué existe y qué no de manera no segura.
```
