# Arquitectura Técnica

## 1. Backend
### searcher-rs
Motor de detección y priorización inicial.
Responsabilidades:
- escuchar mempool / señales
- parsear calldata
- detectar patrones
- construir oportunidades candidatas
- publicar en cola interna

### selector-api
Capa de decisión.
Responsabilidades:
- aplicar reglas de riesgo
- enriquecer con datos de liquidez
- calcular score de profitabilidad
- decidir si simular / descartar

### sim-ctl
Controlador de simulación.
Responsabilidades:
- correr simulaciones fork / sandbox
- devolver gas esperado, slippage, revert risk
- producir resultado determinista para decisión

### relays-client
Capa de ejecución privada.
Responsabilidades:
- seleccionar relay / builder
- firmar bundles / payloads
- reintentos y reemplazos
- trazabilidad de ejecución

### recon
Reconciliación y cierre financiero.
Responsabilidades:
- comparar expected vs actual
- registrar variance
- alimentar learning loop

## 2. Edge
Responsabilidades:
- auth
- rate limiting
- caché de lectura
- sanitización
- streaming de datos no sensibles

## 3. Frontend
Responsabilidades:
- observabilidad
- control operativo
- configuración
- incident timeline
- vista de riesgo

## 4. Automatización
Responsabilidades:
- despliegue
- health checks
- backfills
- backups
- smoke tests
