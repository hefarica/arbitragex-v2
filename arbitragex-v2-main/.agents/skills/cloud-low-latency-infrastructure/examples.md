# Ejemplos seguros

## Optimización de sysctl.conf (Simulación local o VPS)
Este ejemplo muestra parámetros de red seguros para reducir la latencia de establecimiento de conexión.

```bash
# /etc/sysctl.conf
# Deshabilitar métricas TCP para evitar latencia introducida por el SO
net.ipv4.tcp_no_metrics_save=1
# Habilitar TCP Fast Open
net.ipv4.tcp_fastopen=3
# Reducir keepalive time para detectar conexiones muertas más rápido
net.ipv4.tcp_keepalive_time=60
net.ipv4.tcp_keepalive_intvl=10
net.ipv4.tcp_keepalive_probes=6
```

## Pruebas de Latencia Seguras
Hacer un script que mida la latencia al RPC sin enviar transacciones:
```bash
ping -c 100 rpc.example.com | tail -1 | awk '{print $4}' | cut -d '/' -f 2
# Mide el ping promedio y verifica que sea menor a un umbral (ej. 5ms).
```
