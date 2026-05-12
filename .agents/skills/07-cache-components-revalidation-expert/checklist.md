# Checklist Operativo: Caching & Revalidation

- [ ] ¿Los fetch de datos semi-estáticos incluyen un array de tags para revalidación quirúrgica? (Ej: `next: { tags: ['settings'] }`).
- [ ] En las mutaciones (POST/PUT), ¿se invoca a `revalidatePath` o `revalidateTag` inmediatamente tras la confirmación de base de datos?
- [ ] En pantallas dinámicas, ¿está explícitamente desactivada la caché de red (`cache: 'no-store'`)?
