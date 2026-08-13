## Description
<!-- Provide a clear and concise description of the changes -->

## Type of Change
- [ ] feat: new feature
- [ ] fix: bug fix
- [ ] docs: documentation only
- [ ] perf: performance improvement
- [ ] security: security fix
- [ ] refactor: code restructuring
- [ ] test: adding or updating tests
- [ ] ci: CI/CD changes
- [ ] chore: maintenance tasks

## Checklist
- [ ] Tests added or updated (unit + integration + E2E)
- [ ] Lint passes (`npm run lint` + `cargo clippy`)
- [ ] Type check passes (`npm run typecheck`)
- [ ] Build passes (`npm run build` + `cargo build`)
- [ ] Security audit passes (`npm audit` + `cargo audit` + `gitleaks`)
- [ ] Documentation updated (if applicable)
- [ ] Breaking changes documented (if applicable)
- [ ] Scoreboard recalculated if maturity affected

## Related Issues
<!-- Link any related issues: Fixes #123, Closes #456 -->

## Evidence
<!-- Screenshots, logs, traces, or other proof of correctness -->

---

## 🜂 OMEGA Hardening — Embudo de Necesidad (P-∅)
> Doctrina: `docs/governance/HARDENING_ANTI_REGRESION.md`. La carga de la prueba
> es del CAMBIO, no del sistema. Un PR sin anomalía que curar se rechaza por
> incompleto. Responder las 5 + el checklist (un PR = UN ID).

**Anomalía que cura (ID del tracker o L4 con timestamp):**
<!-- ej: R6-04, PIPELINE-0, C-03. "Mejora/refactor/cleanup" sin anomalía → no entra. -->

**¿Qué pasa si NO se hace?**
<!-- "nada observable" → no entra (backlog). "degrada en el futuro" → backlog con fecha. -->

**Archivos tocados (¿alguno de Nivel 1/2 — congelación?)?**
<!-- Lista. Nivel 1 = intocale; Nivel 2 = justificación doble + revisión humana. -->

**Guardianes del baseline afectados + chequeo antes/después:**
<!-- ej: Guardián 1 (feed) — antes: curl X, después: curl Y -->

**Revert (una línea):**
<!-- `git revert <sha>` limpio. Migraciones irreversibles → diseño aparte. -->

### Checklist Hardening
- [ ] Cura UN solo ID (sin "de paso", sin reformateo ajeno, sin deps mezcladas)
- [ ] Ningún archivo de Nivel 1 tocado; Nivel 2 justificado
- [ ] Contract tests verdes · Gate de paridad verde · CI verde (14 required checks)
- [ ] Revert declarado (`git revert` limpio)
- [ ] L4 post-deploy planeado (guardianes + páginas del área; barrido 56 páginas si toca edge/frontend)
- [ ] Tracker: fila abierta/actualizada <24h
