# /goal — FRONTEND DOCTRINE: la DApp perfecta (orden del operador 2026-09-07)

> "Analiza todo el Frontend de 0 a 57 páginas, entienda cómo se relaciona cada página, encuentre
> bugs, documéntelos, analice la mejor estrategia y esquema de una DApp de este nivel enterprise
> Premium, adapte o corrija en este plan, sugiera pequeñas mejoras, estandarice operaciones,
> funcionamiento, carga, streaming actualizado vía snapshot y SOLO actualice el número que
> cambie para que sea más rápido y consuma menos recursos. Frontend de vanguardia, adelantado a
> su tiempo. Este plan será construido por todos y para todos en función de la DApp perfecta."
> — Operador

## Kanban

| WO | Ítem | Acción | Estado |
|---|---|---|---|
| FE-01 | **Censo 0→57 páginas**: cada página/ruta con propósito, fuentes de datos (API/WS/SSR snapshot), componentes clave, estados (loading/empty/error), y el **mapa de relaciones** (navegación + dependencias de datos entre páginas — quién consume qué de quién) | Censo + mapa visual (mermaid) | ENTREGADO 2026-09-07 → FE-01-DESIGN.md · cifra adjudicada **58** (18 client + 28 R1-snapshot + 12 server; "57" del goal = UN, drift histórico 56→58 documentado) · 12 huérfanas del sidebar (/control sin link entrante) · 0 route.ts handlers · mapa mermaid 2 aristas (nav/datos) + tabla 58 filas |
| FE-02 | **Caza de bugs cross-page**: hydration mismatches, estados vacíos no-honestos, WS desconectado silencioso, filtros que desaparecen rows (¡BR-11: hops >2 al aparecer USD!), accesibilidad, overflow, memoria (tickers/leaks del pasado) — cada bug documentado con file:line + repro | Auditoría + docs | PENDIENTE |
| FE-03 | **Doctrina de arquitectura enterprise-premium**: el mejor esquema para ESTA dapp — (a) patrón canónico de datos: **SSR snapshot inicial + streaming delta donde SOLO se actualiza el número que cambió** (no re-render de listas enteras: diff por campo en el store, DOM quirúrgico); (b) estado global estándar (un solo patrón, no N); (c) estándares de loading (skeletons uniformes), error (fail-honest visible), empty states (razón siempre); (d) presupuesto de rendimiento (bundle, re-renders/s, memoria); (e) vanguardia: qué técnicas modernas (React 19/useOptimistic si aplica, streaming SSR, edge caching de estáticos) suman SIN reescribir el mundo | Doctrina con evidencia y diffs concretos | PENDIENTE |
| FE-04 | **EL PLAN (por todos, para todos)**: compone FE-01+02+03 en `FRONTEND-DOCTRINE.md` — el documento que TODA la mesa redonda adopta: mejoras pequeñas priorizadas, estandarizaciones exactas (con diffs), el esquema delta-streaming especificado a nivel componente, y el roadmap de aplicación incremental sin big-bang. Este plan se distribuye a todos los agentes PhD y rige los PRs frontend futuros | Síntesis + distribución | ENTREGADO 2026-09-07 → FE-04-DESIGN.md + **FRONTEND-DOCTRINE.md en raíz** (4 pares leídos, 0 DEPENDENCIA-PENDIENTE; P0×5+P1×7+P2×5 con diffs; checklist ★ PR-validable; delta-streaming por componente con store re-keyed POR RUTA (composición FIX-1×SDQ); 6 olas con gate+rollback; contrato de adopción §7; BR-11 aceptación §5 con canon financing 4 tokens; CB-03 /control des-huérfana P0-5; conflicto websocket-client FE-03×FE-02a adjudicado con evidencia HEAD 27aca289) |
| FE-05 | **Verificación browser** (orquestador): páginas clave con la doctrina aplicable hoy + evidencia del antes | Chromium | PENDIENTE |

## Reglas
- ORDEN SAGRADO: local → repo → VPS → dominio vivo (verificado en Chromium).
- RULE 00/R8: sin datos decorativos; el delta-streaming JAMÁS inventa números — sólo propaga cambios reales del servidor.
- Pequeño gang (≤10 agentes confiables): ecc:react-reviewer, ecc:typescript-reviewer, ecc:a11y-architect, ecc:performance-optimizer, frontend-architect.
- Los hallazgos alimentan BR-11 (dinero por hop) y CB-03 (/control): sincronía de mesa — citar a los pares.
