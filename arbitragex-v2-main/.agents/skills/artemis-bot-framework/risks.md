# Riesgos

- **Riesgos técnicos**: Si se usan canales MPSC en vez de Broadcast para el Engine, la primera Estrategia que lea el evento lo "robará" y otras no lo procesarán.
- **Riesgos de infraestructura**: Ninguno derivado de esta arquitectura de software.
- **Riesgos de red**: Separar Collectors y Executors en servidores distintos requeriría buses de eventos en red (Kafka/Redis), lo cual introduce altísima latencia, por eso Artemis asume que corren en el mismo proceso (RAM compartida).
- **Riesgos de datos**: No tener control sobre la secuencialidad. A veces una Strategy puede requerir eventos en orden absoluto.

## Mitigaciones
- Diseñar el motor interno utilizando canales de tipo Broadcast o canales Multi-Producer Multi-Consumer (MPMC) como `crossbeam-channel` para rendimiento sin lock en sistemas compartidos.
