# ENMIENDA VINCULANTE C9 — MIRROR LAW EXTENDIDA + OPERATOR PARAMETRIZATION SOVEREIGNTY

**Sello:** `Ω-1pct-7c8e3a02-C9-2026-05-14T11:30−05`
**Carácter:** Vinculante, no opcional, integrada al Super-Prompt Ω-S5++.∞ como cláusula adicional **C9**, con prioridad jerárquica equivalente a **L6 (Mirror Fidelity)** dentro de las 7 Leyes Inviolables.
**Lenguaje:** Español. Lexicón Absoluto vigente. Doctrina OMEGA intacta.

---

## C9.1 — Principio de Conservación Estética (Mirror Law Extendida)

Toda modificación al frontend está sometida a un **operador de proyección estética** `P̂_estilo` tal que, para cualquier estado del DOM `|ψ⟩`:

\[
P̂_{estilo} \, |ψ_{nuevo}\rangle = |ψ_{nuevo}\rangle \quad \Longleftrightarrow \quad \langle ψ_{nuevo} | \hat{T}_{tema} | ψ_{nuevo} \rangle = \langle ψ_{actual} | \hat{T}_{tema} | ψ_{actual} \rangle
\]

donde `T̂_tema` es el observable de **invariante estilístico** definido por:

- **Tokens de diseño existentes** (colores, tipografía, spacing, radius, shadows) — congelados en `tailwind.config.ts` y `globals.css`.
- **Componentes shadcn/ui** ya instanciados — congelados en `components/ui/`.
- **Layout root** (`layout.tsx`, providers, navegación) — congelado.
- **Patrones de composición** ya establecidos en las 18+ rutas pre-existentes (Card → CardHeader → CardContent, Tabs, Dialog, Form patterns).

### Regla operativa
Toda página, panel, modal, drawer o componente nuevo:
1. **DEBE** componerse **exclusivamente** de primitivas shadcn/ui ya presentes en `components/ui/`.
2. **DEBE** consumir tokens del tema vigente vía clases Tailwind ya definidas — **prohibido** introducir nuevos colores, fuentes, escalas, o variables CSS.
3. **DEBE** replicar el patrón visual de las páginas hermanas del mismo nivel jerárquico (sidebar, header, breadcrumbs, footer).
4. **PROHIBIDO** instalar nuevas dependencias de UI, frameworks de estilo paralelos, o sobrescribir clases utilitarias con `!important` o CSS custom.

### Verificación cuantitativa
Test obligatorio adicional al suite de 20:

```ts
// e2e/style_invariance.spec.ts
test('C9.1 — Style Invariance Hermiticity', async ({ page }) => {
  // 1. Snapshot del CSS computado de elementos canónicos en rutas pre-existentes
  const baselineTokens = await captureDesignTokens(page, [
    '/dashboard', '/chains', '/strategies'  // rutas pre-S5
  ]);

  // 2. Snapshot de las rutas nuevas /omega-s5/*
  const extendedTokens = await captureDesignTokens(page, [
    '/omega-s5/factory', '/omega-s5/wallets', '/omega-s5/core',
    '/omega-s5/adapters', '/omega-s5/crucible', '/omega-s5/operator',
    '/omega-s5/drift', '/omega-s5/registry'
  ]);

  // 3. Hermiticidad: distancia espectral en espacio de tokens = 0
  expect(spectralDistance(baselineTokens, extendedTokens)).toBe(0);
});
```

**PASS iff** `spectralDistance = 0`. Cualquier desviación = `BLOCKED` y reversión inmediata.

---

## C9.2 — Principio de Reflejo Funcional Total (100% Mirror Surface)

El frontend **DEBE** ser un **reflejo holográfico** del 100% de las utilidades y funcionalidades del backend. Formalmente, si `F_backend = {f₁, f₂, ..., fₙ}` es el conjunto de capacidades expuestas por el backend (endpoints, eventos WSS, registries, gates, agentes, channels de reload, observables de drift, telemetría), entonces:

\[
\forall f_i \in F_{backend}, \; \exists u_i \in F_{frontend} \; : \; \pi(u_i) = f_i
\]

donde `π` es el morfismo de exposición operacional.

### Operacionalización
1. **Endpoint `/api/system/feature_manifest`** (ya implementado en `system-manifest.ts`) es la **única fuente de verdad** de las capacidades activas.
2. El frontend **DEBE** consumirlo en boot y renderizar UI para **cada** entrada con `enabled=true`.
3. Cualquier capacidad backend sin contraparte UI = **violación L6** = `BLOCKED`.
4. Cualquier UI sin respaldo en el manifest = **violación L7 (ghost UI)** = `BLOCKED`.

### Test obligatorio
```ts
test('C9.2 — Total Functional Mirror', async ({ page, request }) => {
  const manifest = await request.get('/api/system/feature_manifest').then(r => r.json());
  const enabledFeatures = manifest.features.filter(f => f.enabled);

  for (const feature of enabledFeatures) {
    const uiSurface = await page.locator(`[data-feature="${feature.key}"]`).count();
    expect(uiSurface, `Feature ${feature.key} sin reflejo UI`).toBeGreaterThan(0);
  }

  // Inverso: no UI ghost
  const declaredUI = await page.locator('[data-feature]').evaluateAll(els =>
    els.map(e => e.getAttribute('data-feature'))
  );
  for (const key of declaredUI) {
    expect(enabledFeatures.map(f => f.key), `UI ${key} sin respaldo manifest`).toContain(key);
  }
});
```

---

## C9.3 — Frontend como Extensión Soberana del Operador

El frontend **NO** es una vitrina pasiva. Es la **superficie de comando** desde la cual el operador:

1. **Agrega** entidades a cualquiera de los 12 registries (Chain, Rpc, Dex, Token, Pool, Wallet, Strategy, Contract, RiskGate, CapitalGate, Relay, Agent).
2. **Modifica** parámetros operativos en caliente (cap USD, slippage máximo, gas limits, timeouts, prioridades).
3. **Activa/desactiva** features del manifest.
4. **Promueve** entidades entre fases (testnet → crucible → mainnet) sometido a Crucible Sovereignty.
5. **Firma** escalaciones de capital con clave criptográfica (cap $0.00 → cap operativo).
6. **Observa** el `drift_observations` en tiempo real con capacidad de remediación in-place.

### Requisito de paridad CRUD
Para los **12 registries**, cada página `/omega-s5/<registry>/` **DEBE** exponer:
- **Listado** paginado con filtros (status, chain_id, enabled).
- **Detalle** con todas las columnas de la tabla.
- **Crear** vía formulario validado con schemas Zod derivados del schema PG.
- **Editar** con `Idempotency-Key` header autogenerado.
- **Soft-delete / disable** con audit trail visible.
- **Hot-reload trigger** con visualización del `runtime_ack` en vivo.
- **Drift panel** local mostrando observaciones específicas del recurso.

### Test obligatorio
```ts
test('C9.3 — Operator CRUD Sovereignty per registry', async ({ page }) => {
  const registries = [
    'rpc', 'contract', 'risk-gate', 'capital-gate', 'relay', 'agent',
    'chain', 'dex', 'token', 'pool', 'wallet', 'strategy'
  ];

  for (const reg of registries) {
    await page.goto(`/omega-s5/registry/${reg}`);

    // Las 7 capacidades obligatorias
    await expect(page.getByTestId(`${reg}-list`)).toBeVisible();
    await expect(page.getByTestId(`${reg}-create-btn`)).toBeEnabled();
    await expect(page.getByTestId(`${reg}-edit-btn`).first()).toBeEnabled();
    await expect(page.getByTestId(`${reg}-disable-btn`).first()).toBeEnabled();
    await expect(page.getByTestId(`${reg}-reload-btn`)).toBeEnabled();
    await expect(page.getByTestId(`${reg}-audit-trail`)).toBeVisible();
    await expect(page.getByTestId(`${reg}-drift-panel`)).toBeVisible();
  }
});
```

---

## C9.4 — Parametrización por Operador (Operator Parametrization Sovereignty)

Cada operador autenticado en la plataforma es un **observable individual** con su propio estado cuántico de parametrización `|operador_i⟩`. La plataforma **DEBE** sostener:

### Esquema de datos requerido
```sql
-- Migración 068 (adicional, a generar si no existe)
CREATE TABLE IF NOT EXISTS operator_parametrization (
  operator_id        TEXT PRIMARY KEY,
  display_name       TEXT NOT NULL,
  signing_pubkey     TEXT NOT NULL UNIQUE,
  role               TEXT NOT NULL CHECK (role IN ('observer','steward','sovereign')),
  cap_usd_ceiling    NUMERIC(20,6) NOT NULL DEFAULT 0.00,
  allowed_chains     JSONB NOT NULL DEFAULT '[]'::jsonb,
  allowed_registries JSONB NOT NULL DEFAULT '[]'::jsonb,
  ui_preferences     JSONB NOT NULL DEFAULT '{}'::jsonb,
  feature_overrides  JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  config_hash        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS operator_session_state (
  session_id         TEXT PRIMARY KEY,
  operator_id        TEXT NOT NULL REFERENCES operator_parametrization(operator_id),
  active_chain_id    INTEGER,
  active_registry    TEXT,
  filters_state      JSONB NOT NULL DEFAULT '{}'::jsonb,
  last_seen          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### Reglas de soberanía
1. **`role='observer'`** → solo lectura del manifest + observabilidad. Cero capacidad de mutación.
2. **`role='steward'`** → puede mutar registries dentro de `allowed_registries` y `allowed_chains`. NO puede subir `cap_usd_ceiling`.
3. **`role='sovereign'`** → único autorizado a firmar escalación de capital, promoción a mainnet, y modificación de `feature_manifest`.

### Aplicación UI
- El frontend **DEBE** consultar `/api/operator/me` en boot y aplicar **gates declarativos** en cada acción.
- Botones, formularios y rutas se **renderizan condicionalmente** según `role` + `allowed_*`.
- `ui_preferences` (densidad de tabla, idioma `es`/`en`, theme — pero sólo entre temas pre-existentes, sin nuevos), persiste entre sesiones.
- `feature_overrides` permite al operador soberano ocultar/exponer features por sesión sin tocar manifest global.

### Test obligatorio
```ts
test('C9.4 — Operator Parametrization Sovereignty', async ({ page, request }) => {
  // Observer no debe ver botones de mutación
  await loginAs(page, 'observer_user');
  await page.goto('/omega-s5/registry/rpc');
  await expect(page.getByTestId('rpc-create-btn')).toBeHidden();
  await expect(page.getByTestId('rpc-edit-btn').first()).toBeHidden();

  // Steward ve CRUD pero no escalation
  await loginAs(page, 'steward_user');
  await page.goto('/omega-s5/registry/rpc');
  await expect(page.getByTestId('rpc-create-btn')).toBeVisible();
  await page.goto('/omega-s5/operator');
  await expect(page.getByTestId('capital-escalation-btn')).toBeHidden();

  // Sovereign tiene control total
  await loginAs(page, 'sovereign_user');
  await page.goto('/omega-s5/operator');
  await expect(page.getByTestId('capital-escalation-btn')).toBeVisible();
});
```

---

## C9.5 — Integración con la Cadena de Coherencia 7-Layer

C9 **NO** sustituye la regla 7-Layer; la **extiende** con dos capas adicionales específicas del operador, formando la **9-Layer Coherence Rule** para acciones autenticadas:

```
Frontend → API → Handler → PG/Redis → Hot-reload → runtime_ack → Audit/Readiness
                                  ↘                                      ↗
                       Operator Authz Layer ←———————— Operator Audit Layer
```

- **L8 Operator Authz**: validación de pubkey + role + allowed_* antes de Handler.
- **L9 Operator Audit**: registro en `audit_event` con `operator_id` y `signing_pubkey` además del payload.

Una acción operacional sin L8 = `BLOCKED`. Una acción sin L9 = `PARTIAL` (auditoría incompleta).

---

## C9.6 — Detonador Actualizado

El comando detonador se mantiene **`Ω-S5++ EJECUTA`** pero ahora desencadena el protocolo de 14 olas Ψ.0 → Ψ.13 **PLUS** dos olas adicionales:

- **Ψ.14 — Style Invariance Sweep**: ejecuta `e2e/style_invariance.spec.ts`, valida hermiticidad estilística, emite reporte de tokens.
- **Ψ.15 — Operator Sovereignty Wiring**: genera migración 068, expone `/api/operator/me`, implementa gates declarativos en las 12 rutas de registry, valida con `e2e/operator_*.spec.ts`.

El reporte final (Cláusula C5) incorpora dos secciones nuevas:
- **§13 — Evidencia de Mirror Law Extendida** (tabla baseline vs extended tokens, distancia espectral).
- **§14 — Matriz de Soberanía del Operador** (3 roles × 12 registries × 7 capacidades = 252 celdas con estado PASS/BLOCKED).

---

## C9.7 — Criterio de Éxito Reformulado

La función de partición se actualiza:

\[
Z = \exp\left(-β \cdot \left[E_{total} + λ_{estilo} \cdot ||\Delta T̂||^2 + λ_{operador} \cdot \sum_i \mathbb{1}[\text{operador}_i \text{ sin gate}]\right]\right)
\]

**PASS iff `Z = 1`** ⟺ `E_total = 0` ∧ `||ΔT̂|| = 0` ∧ ningún operador sin gates correctos.

Cualquier valor `Z < 1` = `BLOCKED`. No hay PASS parcial.

---

## C9.8 — Cláusula de Inviolabilidad

Esta enmienda C9 se integra a las **7 Leyes Inviolables** elevándolas a **9 Leyes Inviolables**:

- **L1** Norm Preservation
- **L2** Schema Hermiticity
- **L3** Causality
- **L4** Idempotency
- **L5** Spatial Isolation
- **L6** Mirror Fidelity (ahora reforzada por C9.1 + C9.2)
- **L7** Ghost Invariant
- **L8** Style Invariance Hermiticity (C9.1)
- **L9** Operator Sovereignty (C9.4)

Cualquier violación de L8 o L9 dispara `Eigenstate Collapse` con reversión transaccional inmediata.

---

**Firma de enmienda:** `Ω-S5++.C9-7c8e3a02`
**Estado:** Vigente, vinculante, integrada al Super-Prompt Ω-S5++.∞.
**Próxima invocación esperada:** `Ω-S5++ EJECUTA` ahora ejecuta 16 olas (Ψ.0 → Ψ.15) con los dos tests adicionales obligatorios (`style_invariance.spec.ts`, `operator_sovereignty.spec.ts`) sumando un total de **22 tests obligatorios** (los 20 originales + 2 de C9).
