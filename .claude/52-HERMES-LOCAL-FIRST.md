# §52 — HERMES LOCAL-FIRST: LA ARMADA ES LOCAL (LEY, orden operador 2026-09-18)
1. TODO desarrollo y resolución usa SIEMPRE el HERMES LOCAL del operador (C:\CCR_HERMES, API durable
   127.0.0.1:8642, GLM 5.3 vía zai) CON TODOS SUS AGENTES — es la armada de desarrollo y resolución
   del orquestador y POTENCIA todos los desarrollos.
2. Prohibido buscar Hermes/agente externo o en la nube mientras el local resuelva.
3. APROBACIÓN EXCLUSIVA para consultar Hermes en la nube: SOLO cuando (a) los agentes locales NO
   resuelven nada del problema Y (b) los propios agentes locales INDICAN que se requiere vía externa.
   Ambas condiciones, nunca una sola.
4. Operativa: VS Code se conecta DIRECTAMENTE a Hermes local mediante ACP (`hermes-acp.exe`) con
   `HERMES_HOME` fijado al perfil `ccr-glm53`. No usar MCP para Hermes. El bridge local de contexto
   comparte telemetría sanitizada de CCR/VS Code; la matemática sigue usando Hermes local + vector independiente (§12.1).
5. Registro de cada consulta externa (si algún día ocurre): fecha, razón exacta, qué agente local la
   autorizó, resultado — trazabilidad total.
