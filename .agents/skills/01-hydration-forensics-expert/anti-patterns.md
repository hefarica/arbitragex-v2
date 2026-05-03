# Antipatrones Prohibidos

## Antipatrón 1: Date.now() en el render root
Esto causa el infame React Minified Error #425 (Text mismatch).

```tsx
// 🔴 PROHIBIDO
export function LiveClock() {
  const [now, setNow] = useState<number>(Date.now()); // El server captura un timestamp, el cliente captura otro.

  return <div>{now}</div>; // Mismatch de texto garantizado.
}
```

## Antipatrón 2: Window Guarding Deficiente
Esto causa el Error #418 (Server UI no coincide con Client UI).

```tsx
// 🔴 PROHIBIDO
export function ResponsiveMenu() {
  // En SSR esto evalúa a false. En el cliente asume false inicialmente.
  // Pero si intentas renderizar basado en esto de inmediato sin usar useEffect:
  const isMobile = typeof window !== 'undefined' ? window.innerWidth < 768 : false;
  
  return (
    <div>
      {isMobile ? <MobileMenu /> : <DesktopMenu />}
    </div>
  );
}
// El servidor renderiza <DesktopMenu />. 
// El cliente en móvil detecta true e intenta renderizar <MobileMenu /> en el primer render, rompiendo la hidratación.
```
