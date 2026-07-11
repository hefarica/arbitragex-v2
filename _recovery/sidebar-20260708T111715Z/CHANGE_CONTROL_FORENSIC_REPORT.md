# CHANGE_CONTROL_FORENSIC_REPORT
## Sidebar Recovery — 2026-07-08

---

## 1. Estado Actual

- **Archivo actual**: `frontend/components/app-sidebar.tsx` (222 líneas)
- **Estado**: Baseline sano (commit 37fca6f) — funcional pero sin mejoras premium
- **Git status**: Limpio en frontend, modificaciones solo en backend
- **Servidor**: Puerto 5173 funcional

---

## 2. Base Sana Detectada

| Candidato | Líneas | Estado | Descripción |
|-----------|--------|--------|-------------|
| `37fca6f` | 222 | ✅ GOOD_BASELINE | Sidebar colapsable básico, funcional |
| `HEAD` | 222 | ✅ GOOD_BASELINE | Idéntico a 37fca6f |
| `45647b0` | 481 | ⚠️ PARTIAL_GOOD | FASE OMEGA: StatusStack + acordeones, pero hydration errors |
| `backup-components` | 358 | ⚠️ PARTIAL_GOOD | StatusStack sin acordeones |
| **stash@{1}** | ~290 | ✅ **BEST_CANDIDATE** | Acordeones limpios, sin StatusStack, sin hydration errors |

**Recomendación**: Usar **stash@{1}** como base para reconstrucción.

---

## 3. Cambios Útiles Detectados

### Desde stash@{1} (excelente):
- ✅ Hook `useCollapsedSections` para acordeones
- ✅ Persistencia `arbx:sidebar:collapsed-sections` en localStorage
- ✅ Headers de sección como botones con chevron (rotación 90°)
- ✅ CSS grid trick para animar altura (0fr → 1fr)
- ✅ Estados colapsados por defecto (operator request)
- ✅ Active state con bg-primary sólido (no glass)
- ✅ Sin backdrop-blur (matte sidebar per operator)

### Desde 45647b0 (con problemas):
- ⚠️ StatusStack completo pero causa hydration errors
- ⚠️ Efectos glassmorphism excesivos
- ⚠️ 481 líneas = más complejo de mantener

---

## 4. Cambios Malos Detectados

### Desde stash@{0} (descartar):
- ❌ Cambios masivos en globals.css (tema blanco puro)
- ❌ Cambio de fuentes globales (Inter, Space Mono)
- ❌ 659 líneas de diff en 9 archivos — demasiado invasivo
- ❌ Rompe el tema oscuro existente

### Otros descartes:
- ❌ Reescritura completa de 475 líneas (causó error JSX)
- ❌ Div de extensión mal cerrado
- ❌ Cambios globales en layout/page para "arreglar" sidebar

---

## 5. Stashes Relevantes

```
stash@{0}: cambios-previos-mockup
  └─ ⚠️ INVASIVO — cambia tema global, fuentes, 9 archivos
  
stash@{1}: pre-existing WIP: app-sidebar + tsconfig + Cargo.lock
  └─ ✅ **USAR ESTE** — acordeones limpios, sin side effects
```

---

## 6. Reflog Relevante

- `37fca6f` — baseline sano (collapsible glassmorphism sidebar)
- `45647b0` — FASE OMEGA (StatusStack + acordeones, pero complejo)
- `24d1407` — HEAD actual (revertido a baseline)

---

## 7. Backups Relevantes

| Archivo | Líneas | Estado |
|---------|--------|--------|
| `components_backup/app-sidebar.tsx` | 358 | StatusStack sin acordeones |
| `app_backup/layout.tsx` | ? | Posiblemente relacionado |

---

## 8. Screenshots Relevantes

| Archivo | Descripción |
|---------|-------------|
| `current-sidebar-state.png` | Estado actual (baseline) |
| `sidebar-blur-minimo.png` | Variante con blur mínimo |
| `sidebar-crystal-effect.png` | Variante crystal |
| `sidebar-gradual-transparente.png` | Variante transparente |
| `sidebar-translucido-65.png` | Variante 65% translúcido |
| `status-stack-*.png` | Múltiples variantes del StatusStack |
| `full-page-verification.png` | Verificación full page |

---

## 9. Claude Temp Outputs Relevantes

- `_recovery/sidebar-20260708T111715Z/` — Snapshot completo creado hoy
- Candidates exportados desde git
- Stashes exportados como patches

---

## 10. Versión Candidata Recomendada como Base

**stash@{1}** — `pre-existing WIP: app-sidebar + tsconfig + Cargo.lock`

Razones:
1. ✅ Acordeones implementados limpiamente
2. ✅ Sin StatusStack (evita hydration errors)
3. ✅ Sin cambios globales de tema
4. ✅ Cercano al baseline (solo +68 líneas aprox)
5. ✅ Persistencia localStorage funcionando
6. ✅ Sin glassmorphism excesivo

---

## 11. Lista Exacta de Features a Reimplementar

### Desde stash@{1} (prioridad alta):
1. Hook `useCollapsedSections` con localStorage
2. Botones de sección con chevron rotatorio
3. CSS grid animation (0fr → 1fr)
4. Estados colapsados por defecto
5. Active state sólido (bg-primary)
6. Sidebar matte (sin blur)

### Mejoras adicionales (packages posteriores):
7. Fuente Helvetica/iOS solo en sidebar
8. LED indicador azul/verde pequeño
9. Scale/Lift sutil en hover (1.01, -1px)
10. Márgenes proporcionales refinados

---

## 12. Lista Exacta de Features a NO Reimplementar

1. ❌ StatusStack (causa hydration errors, implementar después con suppressHydrationWarning)
2. ❌ Glassmorphism excesivo (backdrop-blur-xl)
3. ❌ Cambios globales de tema/fuentes
4. ❌ Reescritura completa del archivo
5. ❌ Badges gigantes en headers
6. ❌ Glow excesivo en hover
7. ❌ Scale 1.02+ (deforma layout)
8. ❌ Submenús enormes tipo bloque

---

## 13. Riesgo

| Riesgo | Nivel | Mitigación |
|--------|-------|------------|
| Hydration errors | Medio | No incluir StatusStack todavía, usar suppressHydrationWarning después |
| Tema roto | Bajo | No tocar globals.css |
| Sidebar no colapsa | Bajo | Probar en 5173 antes de declarar listo |
| Acordeones no animan | Bajo | CSS grid trick está probado en stash@{1} |

---

## 14. Plan de Reconstrucción por Paquetes

### Package 0 — Aplicar stash@{1} como base
```bash
git stash branch recovery/sidebar-stash-1 stash@{1}
# O aplicar solo el diff de app-sidebar.tsx
```

### Package A — Fuente Helvetica solo sidebar
- Aplicar fuente vía style prop al aside
- No tocar globals.css

### Package B — Headers badge compactos
- Refinar estilo de botones de sección
- Mantener chevron, ajustar colores

### Package C — LED indicador (opcional)
- Agregar indicador visual sutil de estado
- NO reemplazar el chevron

### Package D — Scale + Lift sutil
- `hover:scale-[1.01]`
- `hover:translate-y-[-1px]`
- Duration 300ms

### Package E — Márgenes proporcionales
- Ajustar pt-*, pb-*, px-*
- Espaciado refinado entre secciones

### Package F — StatusStack (futuro)
- Implementar con suppressHydrationWarning
- O usar approach diferente (client-only render)

### Package G — Webapp testing final
- Screenshots expanded/collapsed
- Validar acordeones funcionan
- Validar persistencia localStorage

---

## 15. Comandos de Recuperación

### Opción A: Aplicar stash@{1} completo
```bash
git stash apply stash@{1}
# Revertir tsconfig.json y next-env.d.ts si es necesario
```

### Opción B: Extraer solo app-sidebar.tsx del stash
```bash
git show stash@{1}:frontend/components/app-sidebar.tsx > frontend/components/app-sidebar.tsx
```

### Opción C: Usar el patch exportado
```bash
cd frontend/components
patch -p4 < ../../../_recovery/sidebar-20260708T111715Z/app-sidebar.stash-1.patch
```

---

## 16. Validación Obligatoria

Después de cada package:

```bash
cd frontend
npx tsc --noEmit --project tsconfig.json
# Debe pasar sin errores
```

Playwright:
- Navegar a http://127.0.0.1:5173
- Screenshot expanded
- Click collapse button
- Screenshot collapsed
- Verificar acordeones funcionan
- Verificar localStorage persiste

---

## 17. Rollback Plan

Si algo falla:

```bash
# Restaurar desde backup
cp _recovery/sidebar-20260708T111715Z/app-sidebar.current.tsx frontend/components/app-sidebar.tsx

# O desde baseline
git restore --source=37fca6f -- frontend/components/app-sidebar.tsx
```

---

## Conclusión

El trabajo útil está en **stash@{1}**. Es una implementación limpia de acordeones sin los problemas de hydration del StatusStack ni los cambios invasivos de tema del stash@{0}.

**Próximo paso**: Aplicar stash@{1} como base y luego iterar con los packages de mejora visual.
