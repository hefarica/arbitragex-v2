# LEARNINGS — Ledger de experiencia auto-aprendida del Gang Omniscience
> **Protocolo (orden del operador 2026-09-07):** TODA invocación de esta skill LEE este archivo
> ANTES de componer cualquier gang — es la experiencia acumulada de todos los agentes que
> vivieron todas las misiones. Al CIERRE de cada misión, el orquestador destila las lecciones
> nuevas y las ANEXA aquí (fecha + misión + lección + cómo aplicarla). Todos aprenden de todos.

## Cómo usar este ledger
1. Antes de despachar: leer completo (es corto por diseño — cada lección es 1-3 líneas accionables).
2. Las lecciones con etiqueta [ARBX] aplican al repo ArbitrageX; las [GEN] aplican a cualquier
   workspace/gang. Las [CCR] son gotchas del runtime (aplican siempre en sesiones CCR).
3. Al cerrar una misión: anexar SOLO lo nuevo y generalizable — nada de narrativa.

---

## Lecciones (ordenadas por misión de origen)

### Misión: Omniscience Integration 2026-09-06/07 (15 WOs + release a producción)
- [GEN] **RESPAWN-2 bajo 429 sistémico AMPLIFICA** — duplicar agentes contra un proveedor saturado genera más 429s (63/71 muertos). El circuit-breaker (oleadas 4 + presupuesto de respawns + tripwire muertes>2×éxitos) es OBLIGATORIO en todo gang >8 agentes.
- [GEN] **El resume con cache es gratis** — `resumeFromRunId` + prompts/args byte-idénticos: lo completado no re-corre. Cambiar `now`/`goal` en args BUSTEA todo el cache.
- [GEN] **`parallel()` espera THUNKS** `() => Promise` — pasar `map(fn)` que devuelve promesas directas = TypeError. Y `Date.now()` está prohibido en scripts de workflow → pacing ESTRUCTURAL (oleadas), no timers.
- [GEN] **Los dobles de CLIENTE deben replicar la forma de respuesta REAL del cliente** — el espío de ioredis devolvía objetos; el real devuelve arrays crudos `[["name",...]]`; la higiene WO-15 fue no-op silencioso en producción hasta que el L4 lo cazó. Test-double = forma real o nada.
- [GEN] **psql `\if` NO evalúa expresiones SQL** — `\if :var = 0` es inválido ("Boolean expected") y la rama else (abortos deliberados) corre INCONDICIONALMENTE. Patrón correcto: `SELECT (cond)::int \gset` + `\if :ok`. Las migraciones con meta-comandos exigen test de APLICACIÓN real (PG vivo), no solo lint.
- [GEN] **Verificación por capas**: applier→verify adversarial→cross→browser. Los crosses PREDIJERON defectos que producción confirmó (hazard F3 = el 503). "Aserciones de agentes ≠ facts" — cada capa re-ejecuta, no hereda.
- [GEN] **deploy.sh `up -d` es solo wrapper con gates** — NO construye imágenes ni corre migraciones. Secuencia canónica: migraciones → build de servicios cambiados → up. Y TODO deploy termina en L4 (SHA anclado + smoke + invariantes) — el L4 cazó 2 bugs que tests locales no podían ver.
- [GEN] **merge-cascade 405**: si main avanzó, el merge falla con "required status checks expected" → `update-branch` API → ronda 2 de CI. Siempre.
- [GEN] **gitleaks escanea el commit de INTRODUCCIÓN** — un fix posterior no remueve el hallazgo del rango; el remedio canónico para falsos positivos es `.gitleaksignore` con el fingerprint.
- [ARBX] **El dominio público = CF tunnel → :5173 directo, nginx BYPASSEADO** — el upgrade wss:// da 502 porque nginx (que SÍ tiene /socket.io/→8080) quedó fuera de la ruta. Remedio: apuntar el public hostname del tunnel a localhost:80 (remediación D-11 = operador, dashboard CF).
- [ARBX] **Colisión de numeración de migraciones**: dos WOs pueden reclamar el mismo número contra snapshots independientes — el orquestador asigna/adjudica vía BOARD (lección 119→120).
- [ARBX] **`schema_migrations` está stale desde 099** — el runner canónico es `database/run_migrations.sh` (idempotente, sin ledger, lock-guarded). No confundir ledger con verdad.
- [ARBX] **Contenedores con nombre compose**: `arbitragex-v2-redis-1` (no `redis`), `arbitragex-v2-postgres-1`. Y `pgrep -f patrón` en ssh SE AUTO-MACHEA (el bash del comando contiene el patrón) — usar `ps aux | grep -v grep`.
- [ARBX] **Las herramientas pueden estar construidas y NUNCA encendidas** (RU-3 route_scanner: meses built, env ausente). Toda auditoría de "qué falta" debe listar los GATES ENV de cada módulo y su estado REAL en el VPS. (Origen del Control Board soberano.)
- [ARBX] **D-SIM-01 (P0)**: 98.2% de rechazos = `strategy_not_simulatable_in_s4` — desajuste detector-simulador, NO falta de mercado (gas 0.087 gwei). Sin cerrarlo, todo lo demás es económicamente irrelevante.
- [ARBX] **La carrera de reserves**: 114/235 pools cacheados, TTL ~30s, evaluar N legs frescas simultáneas decae exponencial con N — por eso 2-hop a veces y 3+ nunca.
- [CCR] **NUNCA pasar `model:` override** — fuerza routing Anthropic y muere 401/400 en CCR. Los agentes heredan el LLM de sesión.
- [CCR] **El registry de agentes es snapshot de session-start** — ediciones de disco aplican desde la PRÓXIMA sesión.
- [CCR] **Windows AppControl 4551** bloquea spawns de cargo test — workaround: `--no-run` + ejecutar el exe directo desde target/debug/deps.
- [CCR] **py en Git Bash lee cp1252** — abrir JSON con `encoding='utf-8'` explícito; y /tmp de bash ≠ %TEMP% de python.
- [GEN] **Dependabot security-updates BYPASEA las ignore-lists** — puede abrir majors contra la política del repo. Freno total: allow_auto_merge=false + security_updates disabled + eliminar dependabot.yml + cerrar cola de PRs. Nada impone normas sin el operador.

### Misión: Cerebro + Control Board 2026-09-07 (en curso)
- [GEN] **ORDEN SAGRADO (operador)**: local (tests) → repo (PR+CI+merge) → VPS (deploy veraz+migraciones+build) → **dominio vivo (verificación Chromium de que salió perfecto)**. Cero saltos de capa; nada se declara hecho sin el paso 4.
- [ARBX] **BR-11**: la API live devuelve multihop (30/100 triangular, viable_only:false) — el "desaparecen cuando hay USD" es del FRONTEND. La tarjeta debe mostrar TODO el dinero configurado + financiamiento con flash-loan en cualquier nombre (TLS/AAVE_FL/BALANCER_FL/V2_FLASH_SWAP) + waterfall por hop + entrega final.

### Misión: Live-Engineering Overlay + Integración 558+560 2026-09-12/13
- [ARBX] Directiva operador: **testnet-live + mainnet-live AUTORIZADO**; instalada `.claude/skills/arbx-live-engineering/` (checksums 19/19) + §10 supersession en omniscience. Activación financiera sigue operador-only.
- [ARBX] **Receta VPS disco-lleno probada (2026-09-13)**: journal vacuum + qb3-check(/tmp snapshot, uniques salvados a archives/) → 2GB → PG arranca SOLO (crash-loop se auto-cura con espacio) → backfill rollup MANUAL (48 buckets/chunk, CHECKPOINT cada 3) → guard=0 → **TRUNCATE crudo RDO = +74GB en 1s** (WAL mínimo, seguro con margen fino). El PURGE batched genera WAL que puede re-matar PG con margen <2GB — con margen fino NUNCA purgar, solo backfill+truncate. disk_guard.sh perdia +x en checkout (chmod en cada deploy).
- [GEN] **El auto-merge 3-way de fixes+ZIP fue semánticamente correcto** (los deltas no se pisaban por línea) pero SOLO el grep de marcadores lo prueba — verificar SIEMPRE los conteos de markers post-merge (aserción ≠ hecho). Conflicto real único: registro dual op_32 → resolución = un brazo canónico (nsga2) + módulo hermano compilable SIN registrar + test reescrito al contrato real leído del código.
- [GEN] Sesión SSH larga con loops SQL puede morir a mitad sin error visible — los pasos posteriores no corren y nadie te lo dice: **verificar el estado REAL (df/count/perm) tras cada bloque**, jamás asumir que el tail del script corrió.
