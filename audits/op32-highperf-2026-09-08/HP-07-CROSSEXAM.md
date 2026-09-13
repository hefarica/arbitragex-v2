# HP-07 — CROSS-EXAMINATION (par adversarial, Gang Omniscience 2026-09-08)

**Objeto**: refutar `HP-07-ECON.md` + `HP-07-DESIGN.md` (re-anclaje económico del SEED).
**Método**: verificación re-ejecutada, no heredada. Leí board completo, SEED 168-189,
BR-01 (números era-2), BR-00-REVERIFY §0, board cerebro :35-42, FINDINGS.md mundial,
doctrina de rutas, los 4 anclajes de código citados — y **re-verifiqué el VPS EN VIVO**
(05:05Z 09-09, read-only: df, docker ps, redis XINFO/XLEN, git rev-parse). Timezone local
confirmada UTC-5 (cronología del reporte consistente: ventana declarada 02:44-03:06Z =
21:44-22:06 local; mtime archivo 22:14 local = 03:14Z — NO backdated).

## 1. Veredicto: **GAPS** (menores — la tesis NO cae; precisión de anclas a corregir)

HP-07 cumple su charter COMPLETO (GOAL-WORKORDERS.md:35): re-anclaje vs modelo del
operador ✓, condiciones por tramo ✓ (§2.2), riesgos validados vs canon mundial ✓ (§4),
veredicto achievable/unachievable con números ✓ (§5.2). La verificación es REAL, no humo:
**re-mediciones propias la corroboran** (abajo). RULE 00/R8 ejemplar (factorización
λ×a×ȳ×W INV-HP07-1, UNKNOWNs declarados, techo≠expectativa). 0 código, 0 git, VPS
read-only, 0/5 HTTP. Sync de mesa correcta: fue el primer reporte de la mesa op32
(mtimes: HP-07-ECON 03:14Z < HP-06-INFRA 03:19Z < HP-01 02:59Z+), cita a pares cerebro
por archivo, y su handoff a HP-06 (storage línea #1, GPU en contra) fue honrado —
HP-06-INFRA.md:20-46 lo corrobora INDEPENDIENTEMENTE (GPU RECHAZADA, mismo ~16GB/día,
~4KB/fila) sin contradecirlo.

## 2. Lo que VERIFICO correcto (re-ejecutado por mí)

- **SEED 168-175**: escalera/GPU-4xA100/MIN_PROFIT $500/break-even mes 6/$5M/ROI
  10.000%/knobs safety — citas textuales exactas (leídas línea a línea).
- **Números heredados de BR-01**: 546,976 entries@12:51Z (:313,:509) · 7,797 sims/4.98h
  (:588) · cobertura 0.94% = 7,797/828,548 filas ventana (:140) — MI réplica: 0.941% ✓ ·
  77.87% S5 (:492) · 41.1% reverted (:595) · 20.8% timeout (:179) · 783×429 (:563) ·
  9,866/3h de 4,269,568 (:602) · 1,020,039/0 passed (:20) · era 12:46:44Z (:9) ·
  "dispatched=254-291" es la era MADURA (:606) — mi sospecha de mala cita (vs :194
  "319-383") se DISUELVE: HP-07 citó era-apropiado.
- **Board cerebro :38-39**: break-even 5%≈$1,170/10%≈$4,650/20%≈$13,950 + gas 0.087 +
  ETH $2,489 + MEV $393M + Titan 53% — cita exacta.
- **Canon mundial** (FINDINGS.md): las 6 citas arXiv existen con los números exactos
  (2401.01622 $132B/11=80% :53 · 2507.13023 $233.8M/19/19mo/top-3=3/4 :53 · 2508.04003
  $14M/mes :43 · 2510.14642 80.93%/56.54% :49 · 2607.20762 2.02bps/$24M :9 ·
  2510.14480 Lean :25).
- **Aritmética re-derivada por mí, 100% consistente**: λ_opp 6,670,913/38.06h=175,276/h ·
  λ_sim 7,797/4.98=1,566/h · las 8 celdas de a-implícita por tramo · la rejilla 4×4 de
  §3 (16/16 celdas) · $5M→$579/h · §4.1 ($12.31M/mes, $648K, $3.08M, $192K, 59%, 22%) ·
  §5.4 (61×, 15-77×, 516×, ȳ≈$0.021) · degradación RU-3 ~53×≈"50×".
- **Anclajes de código**: trading_config.rs:756 `min_profit_usd: 2.0` ✓ EXACTO ·
  trading_config.rs:164 `max_gas_usd: Option<f64>` ✓ · bundle_builder.rs:313
  `min_profit_wei = U256::from(1u64)` ✓ · size_optimizer.rs:524-533 gas_floor_breach
  con kelly_gas_safety_multiplier ✓ · canonical_knobs.rs:40 FINANCING_MODES 4 modos ✓ ·
  doctrina :49-50 financing=dimensión de ruta ✓ · grep knobs safety del SEED = 0 ✓.
- **RE-MEDICIÓN VIVA (05:05Z, mía)**: el incidente P0 fue REAL y ya remediado PARCIAL:
  disco 127G/150G (88%, 18G libres ≈ los ~21.5GB reclaimable que HP-07 midió para
  `builder prune`); postgres/api-server "Up 2 hours (healthy)" (recuperación ~03:00Z,
  inmediatamente post-reporte 03:14Z); emisión REANUDADA (entries-added 7,510,716 =
  +292,827 sobre el congelado 7,217,889); **terminus `arbx:opps:simulated` sigue 0** →
  a=0.000% persiste, el claim central I4 sigue vivo.

## 3. Gaps encontrados (ninguno refutación-grade; todos con fix)

### X-1 [MEDIO] ancla de production-knob cita fixture de TEST
HP-07-ECON §6 (:263): "CANONICAL_REPO real: `max_gas_burn_usd: 100.0`
(risk_ledger.rs:262)". Verificado: **risk_ledger.rs:241 = `#[cfg(test)]`** — :262 es el
helper `fn th()` de tests. El default de PRODUCCIÓN es DB-driven:
registry-engine.ts:601 `max_gas_burn_usd: z.number().nonnegative().default(0)`; el campo
existe en risk_ledger.rs:99. La TESIS sobrevive (un knob gas-burn existe; los $50K/$1M/
500gwei/$50K-día del SEED no existen en ningún lado — grep 0), pero el ancla presenta un
valor de test como knob de producción y los pares posteriores lo propagarán. Fix:
re-anclar a :99 + registry-engine.ts:601 y marcar 100.0 como fixture.

### X-2 [MEDIO] "(con denominador opps: $0.016)" no reconstruible
§5.4 (:243): ȳ≈$0.021 con λ_sim ✓ (mi réplica: $0.02075), pero el paréntesis con
denominador opps da **$1.85e-4** con λ_opp=175,276 — ningún λ medido (1,566/3,289/
175,276/166K) produce $0.016 con $1,170/mes, a=5%, 720h. Número sin derivación o errado.
Fix: corregir o eliminar.

### X-3 [MENOR] "MTBF 38h" y "factor ≥22×" mal derivados
§0 (:42-43): 38h es time-to-FIRST-failure a carga era-2 (n=1), no MTBF; y el "factor
≥22× en uptime" no tiene derivación en el texto (43 min/mes presupuesto vs ¿qué?).
El 38h sí es real (12:46:44Z→02:49Z = 38.04h). Fix: renombrar + derivar el factor o
quitarlo.

### X-4 [MENOR] "0.05% Aave HOY, leído on-chain" sin lectura
§2.2 nota (:118-119): no hay comando cast/on-chain en §7, y mis greps no hallan 0.05% ni
en la doctrina ni en canonical_knobs.rs (fees "SIEMPRE leídos on-chain" = doctrina :42,
sin valor). Es el fee estándar Aave v3 (conocimiento mundial), pero INV-HP07-1 exige
medido o UNKNOWN. Fix: etiquetar CANONICAL_WORLD o leerlo con cast.

### X-5 [COSMÉTICO] ventanas mezcladas en dos derivaciones
(a) I1 resta una lectura de 12:51Z pero divide por el lapso 12:46:44Z→02:49Z (38.04h vs
37.97h correctos; error 0.2%, material cero — pero viola la letra de su propio
INV-HP07-2). (b) §0 bruto/fila divide crecimiento de PG de 4.5 días (67G) entre filas de
solo-era-2 (6.7M) — el rango 5-11KB es defendible (HP-06 midió ~4KB/fila independiente,
BR-01 :89), pero la mezcla de ventanas debe declararse.

## 4. Estado vivo POST-reporte (para el operador y el próximo verificador)

- **I11 YA STALE**: HP-07 midió VPS en e65040f1 (verdad a 03:0xZ); a las 05:05Z el VPS
  corre **3be8274d = main tip de GitHub** (git ls-remote verificado; SHA ausente del
  árbol local feat/hops-live-01). Hay redeploy real (postgres/api recreados ~03:00Z).
  Consecuencia: el gate F0 #2 ("BR-00 desplegado") debe RE-ANCLARSE contra el contenido
  de 3be8274d — la cadena F0 de HP-07 §5.1 es pre-deploy.
- **Remediación del P0 es PARCIAL**: 18G libres con el driver vivo (~16GB/día) →
  horizonte de re-llenado ~1 día a menos que retención/no-persistir-muertos (BR-04)
  aterrice. GATE-HP07-S (14d ≤0) sigue lejano.
- **Atribución por confirmar**: prune (~18G) + redeploy (3be8274d) a las ~03:00Z.
  Si fue operador/orquestador (lo esperado: board línea 51 "PRs del orquestador al
  final"), sin issue. Si lo ejecutó algún agente del gang → violación VPS-mutation/
  NO-GIT a registrar en BOARD. Yo no tengo evidencia de lo segundo; HP-07 fue
  explícitamente read-only ("yo read-only... acciones de operador").

## 5. Para los pares posteriores

- **HP-08-B/browser-verify**: HP-07-ECON §9 te dijo que browser-verify era imposible
  hasta resolver §0 — YA ES POSIBLE (pipeline revivo, 05:05Z): la API sirve y el
  terminus sigue XLEN=0. Ejecuta tu journey pendiente.
- **Quien re-anclre F0**: orden de gates post-3be8274d: (1) disco ✓parcial (88%),
  (2) ¿BR-00 dentro de 3be8274d? (verifica log sim-ctl/boot), (3) S5 (v3_quote_unavailable
  %), (4) signer probe (reverted %), (5) reserves. `a` sigue 0/1,020,039+.
- No contradigo a ningún par: BR-01/BR-00-REVERIFY confirmados; HP-06 corroborado
  independiente; HP-03-CROSSEXAM no intersecta mi WO.

**Conclusión**: HP-07 es el entregable más verificado de la mesa hasta ahora — sobrevivió
re-medición viva y re-derivación aritmética completa. Los 5 gaps son de precisión de
anclas (X-1/X-2 los únicos que importan para no propagar errores). La escalera F3/F4
UNACHIEVABLE por tamaño de mercado y el re-anclaje F0-first quedan EN PIE tras mi ataque.
