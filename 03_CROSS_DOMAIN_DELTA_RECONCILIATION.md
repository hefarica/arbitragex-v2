# [MÓDULO] Reconciliación de Deltas Cross-Domain
**Dominio:** Estabilidad de precios entre L1 y Rollups L2
**Ejecución:** Filtros Estocásticos (Kalman) y Control Plane

### 1. MODELO DE DISCREPANCIA DE ORÁCULOS
Las redes descentralizadas son asíncronas. Un impacto de precio en un dominio (ej. Optimism) tarda en reflejarse en otro (ej. Base o Ethereum L1). Este módulo rastrea las discrepancias de precio ($Delta_{price}$) del mismo activo entre múltiples dominios.

### 2. FILTRADO Y EJECUCIÓN (RUST CORE)
* **Ingesta Estocástica:** El módulo `searcher-rs` utiliza un Filtro de Kalman extendido para limpiar el ruido de las cotizaciones de los RPCs y estimar la trayectoria real del precio.
* **Ejecución Ciega (Zero-Knowledge Recon):** No es necesario decodificar la lógica del *smart contract* que generó la discrepancia. El sistema solo necesita saber que el Precio A > Precio B.
* **Routing Simultáneo:** Se dispara una orden de compra en el manifold de menor valor y una venta en el manifold de mayor valor.