# Antipatrones Prohibidos

## Antipatrón 1: Accidental Static Pages in Operational Panels
```tsx
// 🔴 PROHIBIDO: Next.js evaluará esto y, al no ver un parámetro dinámico ni un export const, 
// compilará el resultado y congelará los datos de la base de datos hasta que hagas un nuevo despliegue.

export default async function ArbitrageOpportunities() {
  const opps = await getOpportunities(); 
  return <div>{opps.length} detectadas</div>
}
```
