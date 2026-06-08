# Índice de fuentes del chat integrado

Esta referencia conserva la procedencia documental usada para construir la skill `arbitragex-v2-omni-ssot-operator`. Cargarla solo cuando se necesite trazabilidad, ampliación de la skill o verificación de qué material fue incorporado.

## Fuentes revisadas

| ID | URL | Contenido incorporado |
|---|---|---|
| F1 | `https://manus.im/share/FMEEl1Hd5sqac8bzF62wNz` | Replay inicial indicado por el usuario. Contiene una tarea de integración de skill relacionada con **ArbitrageX v2**, mapeo Omni-SSOT, validación de frontend/VPS, parser RPC y optimización. |
| F2 | `https://manus.im/share/YPY26LAdlblJNAjXeMMit4` | Replay interno detectado en la cadena. Contiene auditoría frontend, objetivo técnico, fases de trabajo y adjunto asociado al mapeo Omni-SSOT. |
| F3 | `https://manus.im/share/OmFVJGiNPRh8XFOoWOYT05` | Replay fuente titulado “¿Qué necesita este repositorio para operar en vivo?”. Incluye diagnóstico RPC end-to-end y documento descargable. |

## Documento descargado e integrado

El documento **Diagnóstico e Implementación End-to-End: RPC_HTTP_1 y RPC_WS_1** fue descargado desde la fuente F3 en formato Markdown. Su contenido fue integrado en `references/rpc_fail_honest_g_rpc_1.md` y conservado de forma normalizada como fuente de trabajo durante la creación de la skill.

| Documento | Fecha visible | Sistema visible | Contenido clave |
|---|---|---|---|
| Diagnóstico e Implementación End-to-End: `RPC_HTTP_1` y `RPC_WS_1` | 2026-05-23 | ArbitrageX v2 — VPS 195.201.235.70 | R8 Fail-Honest, `paper_mode`, `.env`, Docker Compose, formato G-RPC-1, reinicio seguro y verificación de logs. |

## Contenido doctrinal incorporado

| Doctrina o patrón | Referencia donde quedó integrado |
|---|---|
| Omni-SSOT por dominios DEX, Pools, Assets, Opportunities, Strategies, Wallets, Omega, Apex y WebSocket | `omni_ssot_mapping.md` |
| R8 Fail-Honest y G-RPC-1 | `rpc_fail_honest_g_rpc_1.md` |
| Fase 8 de optimización frontend con lazy loading y build validation | `frontend_performance_phase_8.md` |
| Validación VPS, contenedor frontend, puerto 5173 y parser RPC | `vps_validation_checklist.md` |

## Limitaciones de extracción

La interfaz pública de replay mostró contenido renderizado y documentos adjuntos. Se descargó el documento RPC completo desde la fuente F3. Para F1 y F2, se integró el contenido visible y registrado durante la exploración del replay. Si el usuario proporciona exportaciones adicionales del chat o archivos originales, ampliar la skill agregando referencias específicas en lugar de duplicar información dentro de `SKILL.md`.
