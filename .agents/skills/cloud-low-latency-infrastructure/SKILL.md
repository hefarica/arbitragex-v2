# Arquitectura Cloud de Ultra Baja Latencia

## Nivel
Nivel experto avanzado.

## Propósito
Diseñar infraestructura cloud optimizada para trading de alta frecuencia (HFT) y MEV, reduciendo la latencia de red al mínimo absoluto mediante colocación, bypass del kernel y optimización de topología.

## Fuente de aprendizaje
https://www.alibabacloud.com/blog/a-guide-to-ultra-low-latency-crypto-trading-on-the-cloud-part-1---infrastructure-fundamentals_601851

## Conocimiento interiorizado
El trading de criptomonedas depende de ganar microsegundos.
1. **Zonas Geográficas (Colocation)**: Los principales exchanges están en Tokyo (AWS AP-Northeast-1 para Binance), Irlanda, o US-East. Desplegar la VM en la misma Availability Zone (AZ) del exchange/RPC es mandatorio.
2. **Optimización de Red Cloud**:
   - Evitar NAT gateways y proxies innecesarios.
   - Usar instancias con *Enhanced Networking* (ej. SR-IOV) que evitan que el tráfico pase por el hipervisor de la nube.
   - Usar VPC peering directo si el proveedor RPC soporta endpoints en la misma nube.
3. **Kernel Bypass**: Tecnologías como DPDK permiten a la aplicación leer paquetes directamente de la tarjeta de red (NIC), saltándose el stack TCP/IP del SO.
4. **Jitter**: Reducir el jitter (variación de latencia) aislando CPUs (pinning) en la VM para evitar *context switching*.

## Cuándo activar esta skill
- Al diseñar o auditar el despliegue de producción del `api-server` o `searcher-rs`.
- Al evaluar por qué el bot MEV pierde contra otros competidores que envían la misma transacción más rápido.
- Al configurar Docker/Kubernetes en el VPS de producción.

## Cuándo no activar esta skill
- Entornos de desarrollo local.
- Operaciones off-chain que no compiten por bloque.

## Entradas necesarias
- Ubicación física del proveedor de nodos RPC o exchange.
- Presupuesto de infraestructura.

## Procedimiento paso a paso
1. Identificar la región del nodo RPC (ej. AWS us-east-1).
2. Adquirir VPS/Cloud Instance en esa misma AZ.
3. Configurar la red en modo host (`--network host` en Docker) para evitar el overhead del bridge.
4. Auditar la latencia con `ping` y `traceroute` hacia el RPC. Debe ser < 2ms.
5. Optimizar el Kernel Linux: afinar parámetros sysctl para networking (`tcp_fastopen`, `tcp_nodelay`).

## Salidas esperadas
- Configuración de infraestructura y SO optimizada.
- Documento de topología de red.

## Aplicación al proyecto actual
Aplicable al despliegue del VPS de `ArbitrageX`. El contenedor `searcher-rs` debe estar optimizado y correr sin overhead de red (Docker host mode si es seguro).

## Aplicación a futuros proyectos
Cualquier dApp de trading, orderbooks en tiempo real o nodos de validación.

## Buenas prácticas
- Medir p99 de latencia constantemente.
- Separar el nodo de ejecución (EVM local) en la misma red de área local del bot.

## Errores comunes
- Poner el bot en Europa y el nodo RPC en Estados Unidos (añade ~80-100ms de latencia).
- Usar stacks TCP/IP lentos o lenguajes con Garbage Collection impredecibles (Node.js) para la ejecución final.

## Riesgos técnicos
- Host networking en Docker expone todos los puertos, riesgo de seguridad crítico.
- Tuning extremo de sysctl puede causar inestabilidad del sistema operativo.

## Riesgos legales, éticos o financieros
- Ninguno por la infraestructura en sí.

## Controles de seguridad
- Si se usa Host Network, aplicar iptables rígido a nivel de Kernel (UFW/Firewalld).
- VPN/Wireguard exclusivo para acceso administrativo.

## Checklist operativo
- [ ] Región Cloud validada vs RPC.
- [ ] Latencia medida < 2ms.
- [ ] Docker corriendo en modo bridge optimizado o host (si firewall).
- [ ] CPUs anclados (CPU affinity) para el proceso de Rust.

## Ejemplo seguro
Ver `examples.md`.

## Dependencias
- Conocimientos de Sysadmin Linux, TCP/IP, Docker Networking.

## Métricas de calidad
- RTT (Round Trip Time) hacia el RPC de ejecución on-chain mantenido por debajo de 5ms consistentemente.

## Criterios de finalización
- VPS configurado, métricas de red establecidas y validadas con un script de ping.
