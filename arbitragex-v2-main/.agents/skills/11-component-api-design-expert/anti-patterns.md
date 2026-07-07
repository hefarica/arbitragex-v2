# Antipatrones Prohibidos

## Antipatrón 1: El Componente Frankenstein (Prop drilling infernal)
```tsx
// 🔴 PROHIBIDO
// Añadir props booleanas por cada caso de uso visual mata la escalabilidad.
<Button 
  isRed={true} 
  isLarge={false} 
  hasIconLeft={true} 
  iconLeft={<RefreshCw />} 
  isLoading={false}
  noBorder={true}
>
  Refresh
</Button>
```

## Antipatrón 2: Bloqueo de HTML Props
```tsx
// 🔴 PROHIBIDO
interface CardProps { children: React.ReactNode, cssClass: string }
export function Card({ children, cssClass }: CardProps) {
  // Ignora onClick, aria-labels, IDs...
  return <div className={cssClass}>{children}</div> 
}
```
