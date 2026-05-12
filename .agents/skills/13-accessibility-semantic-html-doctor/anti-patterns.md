# Antipatrones Prohibidos

## Antipatrón 1: Nido Ilegal Causa Hydration Error #425
```tsx
// 🔴 PROHIBIDO: Esto romperá la hidratación en React
export function BadComponent() {
  return (
    <p>
      Bienvenido al sistema.
      <div className="mt-4">
        Tus métricas están cargando...
      </div>
    </p>
  );
}
// El navegador convierte esto en: <p>Bienvenido</p><div...</div><p></p>
// React ve el DOM diferente al SSR y entra en pánico de hidratación.
```

## Antipatrón 2: El Div Interactivo Ciego
```tsx
// 🔴 PROHIBIDO
<div onClick={submitForm} className="btn-primary cursor-pointer hover:bg-blue-600">
  Submit
</div>
// Imposible activar con "Enter" o "Space". No recibe foco natural del "Tab".
```
