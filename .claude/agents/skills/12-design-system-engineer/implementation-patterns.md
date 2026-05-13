# Patrones Correctos (Implementation)

## Patrón 1: CSS Variables Semánticas (globals.css)
```css
@layer base {
  :root {
    --background: 222.2 84% 4.9%;
    --foreground: 210 40% 98%;
    --card: 222.2 84% 4.9%;
    --card-foreground: 210 40% 98%;
    --primary: 210 40% 98%;
    --primary-foreground: 222.2 47.4% 11.2%;
    --destructive: 0 62.8% 30.6%;
    --destructive-foreground: 210 40% 98%;
  }
}
```

## Patrón 2: Consumo Semántico de Tailwind
```tsx
// 🟢 CORRECTO: Uso de semántica.
export function ErrorAlert({ message }) {
  return (
    <div className="bg-destructive/20 border border-destructive text-destructive-foreground p-4 rounded-md">
      {message}
    </div>
  );
}
```
