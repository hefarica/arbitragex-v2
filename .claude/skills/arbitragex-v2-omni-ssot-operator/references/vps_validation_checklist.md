# Referencia de Validación VPS y Producción

Usar esta referencia cuando el usuario pida validar si **ArbitrageX v2** está corriendo en el VPS, revisar contenedores Docker, confirmar frontend en el puerto 5173, validar parser RPC o comprobar que el despliegue quedó funcional.

## Hallazgos integrados del chat

La cadena de replays compartida indica que se validó un componente `TopologyVaultClient.tsx`, incluyendo un parser RPC de **12 proveedores**, y que el contenedor `arbitragex-v2-frontend-1` quedó listo con respuesta en el puerto **5173**. Tratar estos datos como referencia documental y verificarlos contra el entorno real antes de afirmar estado actual.

| Elemento | Evidencia documental | Validación real sugerida |
|---|---|---|
| Frontend container | `arbitragex-v2-frontend-1` | `docker ps` y `docker logs`. |
| Puerto frontend | 5173 | `curl -I http://localhost:5173`. |
| Parser RPC | `TopologyVaultClient.tsx` con 12 proveedores | Revisar archivo y pruebas de parsing. |
| Build | Validación posterior a optimización | Ejecutar script de build real. |
| VPS | Sistema asociado a ArbitrageX v2 | Confirmar hostname, ruta y compose activo. |

## Checklist de verificación

```bash
# Estado general de contenedores
docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'

# Logs recientes del frontend
docker logs --tail=100 arbitragex-v2-frontend-1

# Respuesta local del frontend
curl -I http://localhost:5173

# Buscar TopologyVaultClient y parser RPC
grep -RIn "TopologyVaultClient\|Alchemy\|Infura\|QuickNode\|rpc" /opt/arbitragex-v2 2>/dev/null | head -100
```

Si el nombre del contenedor no coincide, usar `docker ps` para descubrir el nombre correcto. Si el frontend corre detrás de proxy, validar también Nginx, Traefik o el balanceador configurado.

## Validación del parser RPC

Buscar en `TopologyVaultClient.tsx` o archivo equivalente la lista de proveedores soportados. Si el chat menciona 12 proveedores, no asumir que siguen siendo 12; contar los patrones reales y documentar diferencias.

```bash
cd /opt/arbitragex-v2
grep -RIn "TopologyVaultClient" .
grep -RIn "alchemy\|infura\|quicknode\|ankr\|chainstack\|blast\|drpc\|llama\|publicnode" frontend src app components 2>/dev/null
```

## Validación de despliegue frontend

Antes de reiniciar o reconstruir, capturar estado actual. Después de cambios, comparar logs y salud HTTP.

| Fase | Acción | Resultado esperado |
|---|---|---|
| Precheck | `docker ps`, logs y `curl -I`. | Estado base documentado. |
| Build | Ejecutar script oficial del repo. | Build exitoso sin errores críticos. |
| Deploy | Usar script existente de infraestructura. | Contenedores recreados sin huérfanos. |
| Smoke test | `curl`, navegación y logs. | UI responde y no hay errores de runtime. |

## Reglas para producción

No reiniciar contenedores, no editar `.env`, no ejecutar deploy ni modificar archivos en `/opt/arbitragex-v2` sin confirmación. Si el usuario pide solo diagnóstico, limitarse a lectura pasiva y entregar evidencia. Si se observan secretos en logs o archivos, enmascararlos inmediatamente en cualquier reporte.
