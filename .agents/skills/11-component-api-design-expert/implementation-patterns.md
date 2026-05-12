# Patrones Correctos (Implementation)

## Patrón 1: Composición y Slotting (Inversión de Control)
Permite máxima flexibilidad sin ensuciar la API del contenedor.

```tsx
// 🟢 CORRECTO
export function OpportunityCard({ header, body, footer }: { header: ReactNode, body: ReactNode, footer?: ReactNode }) {
  return (
    <div className="border border-slate-800 rounded-xl bg-slate-900/50">
      <div className="border-b border-slate-800 p-4">{header}</div>
      <div className="p-4">{body}</div>
      {footer && <div className="p-4 bg-slate-950/30">{footer}</div>}
    </div>
  );
}

// Uso:
<OpportunityCard 
  header={<h3 className="text-emerald-400">ETH/USDC</h3>}
  body={<Sparkline chartData={data} />}
/>
```

## Patrón 2: Class Variance Authority (CVA) + Tailwind Merge
```tsx
// 🟢 CORRECTO
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold",
  {
    variants: {
      variant: {
        default: "border-transparent bg-slate-100 text-slate-900",
        destructive: "border-transparent bg-rose-500 text-slate-50",
        outline: "text-slate-950",
      },
    },
    defaultVariants: { variant: "default" },
  }
)

export interface BadgeProps extends React.HTMLAttributes<HTMLDivElement>, VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />
}
```
