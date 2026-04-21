# Política de Riesgo

## Límites sugeridos
- pérdida máxima por hora: configurable
- pérdida máxima por día: configurable
- revert rate máxima: configurable
- slippage máximo tolerado: configurable por estrategia
- gas burn máximo por chain: configurable

## Categorías
### Riesgo de infraestructura
- caída de nodo
- jitter elevado
- saturación CPU / RAM
- caché inconsistente

### Riesgo de ejecución
- bundle no incluido
- relay fallando
- bloque reordenado
- nonces en conflicto

### Riesgo de mercado
- liquidez falsa
- token malicioso
- oracle divergence
- sandwich de terceros

## Controles
- whitelist dinámica
- blacklist automática
- simulación previa
- circuit breaker por estrategia
- kill-switch por degradación estadística
