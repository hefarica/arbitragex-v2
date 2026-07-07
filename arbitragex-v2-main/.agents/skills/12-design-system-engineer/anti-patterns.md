# Antipatrones Prohibidos

## Antipatrón 1: Magic Values & Hardcoded Colors
Rompe el Design System, hace imposible soportar temas claros/oscuros o rediseños de marca.

```tsx
// 🔴 PROHIBIDO
export function ErrorAlert({ message }) {
  // Si mañana el cliente decide que el rojo de la empresa es "#ff3333", hay que buscar en todos los archivos.
  return (
    <div className="bg-[#ff0000] text-[#ffffff] p-[15px] border-[#cc0000]">
      {message}
    </div>
  );
}
```
