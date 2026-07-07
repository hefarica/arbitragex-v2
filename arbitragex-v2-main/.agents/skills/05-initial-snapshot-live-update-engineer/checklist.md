# Checklist Operativo: Snapshot + Live Update

- [ ] ¿El frontend obtiene datos útiles antes de que el WebSocket esté "Connected"? (Polling inicial / HTTP GET Inicial).
- [ ] ¿El event handler del socket hace un de-duplication check usando `Array.some(item => item.id === new.id)` antes de agregar?
- [ ] ¿El servidor retiene (en Redis, Memoria o Postgres) un buffer de eventos pasados para que el Snapshot contenga datos relevantes inmediatos?
- [ ] ¿Las oportunidades conservan límite superior (ej: `slice(0, 50)`) para que un socket que envíe 1000 eventos/min no colapse la memoria del navegador?
