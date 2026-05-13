# Checklist Operativo: Real-Time UI Performance

- [ ] ¿El componente de la tabla o feed de oportunidades tiene un límite máximo duro? (ej. `slice(0, 50)`).
- [ ] ¿Las celdas pesadas (gráficos, modales asociados) están envueltas en `React.memo` con comparación estricta de props?
- [ ] ¿Se utiliza CSS Transforms o animaciones opacas para efectos de parpadeo ("Pulse") en lugar de mutar clases mediante React Hooks en cada cuadro?
- [ ] ¿Los arrays recibidos por el WebSocket se despachan en baches (Batches) mediante un worker o se hacen throttle en el reducer del estado si exceden un threshold seguro?
