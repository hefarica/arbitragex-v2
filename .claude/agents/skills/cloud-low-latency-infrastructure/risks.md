# Riesgos

- **Riesgos técnicos**: Configurar mal `sysctl` puede hacer inestable el SO. CPU pinning incorrecto en sistemas hiper-threadeados (HT) degrada rendimiento en vez de mejorarlo.
- **Riesgos de infraestructura**: Bloqueo con un solo proveedor de nube (vendor lock-in) por usar sus redes optimizadas privativas.
- **Riesgos de red**: Pérdida de paquetes si los buffers del anillo de la NIC (Ring Buffers) son muy pequeños para ráfagas MEV.
- **Riesgos financieros**: Altos costes por tráfico saliente cruzando regiones o VPS ultra-potentes.

## Mitigaciones
- Siempre aplicar automatización (Ansible/Terraform) para asegurar reproducibilidad.
- Monitorear descartes de red (`netstat -s | grep "packet receive errors"`).
