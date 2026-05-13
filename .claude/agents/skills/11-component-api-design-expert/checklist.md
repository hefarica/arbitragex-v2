# Checklist Operativo: Component API Design

- [ ] ¿El componente de UI delega las clases externas vía `className` usando la utilidad `cn` (tailwind-merge)?
- [ ] ¿Extiende los atributos HTML estándar correspondientes a su elemento raíz (`HTMLAttributes`) para no bloquear `aria-labels` o `data-testids`?
- [ ] ¿Si el componente tiene variantes visuales (primary, secondary, danger), usa una utilidad como `cva` (Class Variance Authority) en lugar de ternarios encadenados infinitos?
- [ ] ¿Se prefiere la inyección de dependencias visuales (`children`) sobre el paso de JSONs gigantes (`data={enormousObject}`)?
