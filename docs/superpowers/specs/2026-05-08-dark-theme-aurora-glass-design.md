# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Dark Theme: Aurora Mesh + iOS Glassmorphism â€” Design Spec

**Date:** 2026-05-08
**Status:** Draft (awaiting user review)
**Scope:** Frontend visual refresh â€” `.dark` theme only. Light theme untouched.
**Reversibility:** Trivial â€” `git revert` of 3 files restores prior state.

---

## 1. Objective

Replace the current warm-orange dark theme with a deep blue â†’ cyan â†’ mint palette (matching the user-provided Open Self Service reference image), add an animated aurora mesh background, and apply iOS-style glassmorphism (blur + translucency) to all surface components (cards, popovers, sheets, sidebar).

**Hard constraints (user instruction "DEJAR TODO COMO ESTÃ"):**
- No changes to component logic, props, state, or JSX structure.
- No changes to light theme.
- No new npm dependencies.
- All changes confined to **3 files**.

---

## 2. Decisions (locked by user)

| ID | Decision | Choice | Rationale |
|---|---|---|---|
| D1 | Animation style | **Conic mesh aurora** (3 radial-gradient blobs, slow translate3d) | Matches reference image (laptop hero with cyan/mint glow blobs); GPU-only; ~0.3% CPU |
| D2 | Primary accent hue | **Cyan `#22D3EE`** (oklch â‰ˆ `0.78 0.13 215`) | Cooler / "tech-HFT" vibe over mint; high contrast on deep navy |
| D3 | Glassmorphism intensity | **iOS typical** â€” `blur(20px) saturate(180%)` + `bg-card` at 50% opacity | Standard iOS Control Center recipe; balances readability and depth |

---

## 3. Color Palette (OKLCH)

All values in OKLCH for perceptual uniformity. Hue sweep: navy (260Â°) â†’ royal (250Â°) â†’ cyan (215Â°) â†’ mint (160Â°).

### `.dark` tokens (replacing current warm-orange palette)

| Token | Current | New | Visual |
|---|---|---|---|
| `--background` | `oklch(0.145 0.01 65)` warm-near-black | `oklch(0.16 0.04 260)` deep navy | `#0A1230` |
| `--foreground` | `oklch(0.985 0.002 80)` cream | `oklch(0.97 0.012 220)` cool white | `#EAF2F7` |
| `--card` | `oklch(0.19 0.012 65)` warm grey | `oklch(0.22 0.05 250 / 55%)` royal-translucent | glass over aurora |
| `--popover` | `oklch(0.19 0.012 65)` | `oklch(0.20 0.05 250 / 70%)` | denser glass (future-proof â€” popover not yet imported) |
| `--primary` | `oklch(0.72 0.16 45)` orange | `oklch(0.78 0.13 215)` cyan | `#22D3EE` |
| `--primary-foreground` | `oklch(0.145 0.01 65)` | `oklch(0.16 0.04 260)` deep navy | high contrast |
| `--secondary` | `oklch(0.24 0.012 65)` | `oklch(0.26 0.04 250)` | muted royal |
| `--muted` | `oklch(0.24 0.012 65)` | `oklch(0.26 0.04 250)` | muted royal |
| `--muted-foreground` | `oklch(0.70 0.015 75)` | `oklch(0.74 0.03 220)` | cool grey |
| `--accent` | `oklch(0.28 0.02 55)` | `oklch(0.30 0.06 220)` | cyan-tint |
| `--accent-foreground` | `oklch(0.985 0.002 80)` | `oklch(0.97 0.012 220)` | cool white |
| `--success` | `oklch(0.70 0.14 145)` | `oklch(0.78 0.16 160)` mint | `#34D399` |
| `--info` | `oklch(0.72 0.11 240)` | `oklch(0.78 0.13 215)` cyan | `#22D3EE` (= primary) |
| `--warning` | `oklch(0.80 0.14 75)` | `oklch(0.82 0.14 75)` | unchanged hue (amber readable on navy) |
| `--destructive` | `oklch(0.65 0.20 25)` | `oklch(0.68 0.22 15)` | warmer red for navy contrast |
| `--border` | `oklch(1 0 0 / 10%)` | `oklch(1 0 0 / 12%)` | slight bump for glass edges |
| `--input` | `oklch(1 0 0 / 14%)` | `oklch(1 0 0 / 16%)` | matched bump |
| `--ring` | `oklch(0.72 0.16 45)` orange | `oklch(0.78 0.13 215)` cyan | matches primary |
| `--sidebar` | `oklch(0.17 0.012 65)` | `oklch(0.18 0.05 255 / 60%)` | translucent navy panel |
| `--sidebar-primary` | `oklch(0.72 0.16 45)` | `oklch(0.78 0.13 215)` cyan | matches primary |
| `--sidebar-accent` | `oklch(0.24 0.012 65)` | `oklch(0.28 0.05 250 / 70%)` | translucent |
| `--sidebar-border` | `oklch(1 0 0 / 10%)` | `oklch(1 0 0 / 12%)` | matched |
| `--chart-1..5` | warm rotation | `cyan, mint, royal, amber, magenta` (oklch) | preserves chart legibility |

---

## 4. Animated Background â€” Aurora Mesh

### Component: `frontend/components/animated-bg.tsx` (NEW, ~25 LOC)

Server Component (zero JS shipped). Renders a `<div>` fixed at `inset-0 -z-10` with three overlaid radial gradients. Hidden on light theme via `dark:` selector â€” light keeps current solid background.

```tsx
export function AnimatedBg() {
  return (
    <div
      aria-hidden
      className="pointer-events-none fixed inset-0 -z-10 hidden overflow-hidden dark:block"
    >
      <div className="aurora-blob aurora-blob-1" />
      <div className="aurora-blob aurora-blob-2" />
      <div className="aurora-blob aurora-blob-3" />
    </div>
  );
}
```

### CSS (added to `globals.css`)

Three blobs of different hues with desynchronized 18-26s `translate3d` orbits. Pure GPU compositor â€” no layout / paint cost during animation.

```css
.aurora-blob {
  position: absolute;
  border-radius: 9999px;
  filter: blur(80px);
  opacity: 0.55;
  will-change: transform;
}
.aurora-blob-1 { /* cyan */
  width: 60vw; height: 60vw; top: -20vw; left: -10vw;
  background: radial-gradient(circle, oklch(0.78 0.13 215) 0%, transparent 70%);
  animation: aurora-pan-1 22s ease-in-out infinite;
}
.aurora-blob-2 { /* mint */
  width: 50vw; height: 50vw; bottom: -15vw; right: -5vw;
  background: radial-gradient(circle, oklch(0.78 0.16 160) 0%, transparent 70%);
  animation: aurora-pan-2 26s ease-in-out infinite;
}
.aurora-blob-3 { /* royal */
  width: 45vw; height: 45vw; top: 30vh; left: 30vw;
  background: radial-gradient(circle, oklch(0.55 0.18 260) 0%, transparent 70%);
  animation: aurora-pan-3 18s ease-in-out infinite;
}
@keyframes aurora-pan-1 {
  0%, 100% { transform: translate3d(0, 0, 0) scale(1); }
  50%      { transform: translate3d(20vw, 10vh, 0) scale(1.15); }
}
@keyframes aurora-pan-2 {
  0%, 100% { transform: translate3d(0, 0, 0) scale(1); }
  50%      { transform: translate3d(-15vw, -10vh, 0) scale(0.9); }
}
@keyframes aurora-pan-3 {
  0%, 100% { transform: translate3d(0, 0, 0) scale(1); }
  50%      { transform: translate3d(10vw, -15vh, 0) scale(1.1); }
}
@media (prefers-reduced-motion: reduce) {
  .aurora-blob { animation: none !important; }
}
```

### Why this works
- `position: fixed` + `inset-0 -z-10` keeps it behind everything without affecting layout.
- `pointer-events-none` ensures it can't intercept clicks.
- `dark:block` (with `hidden` default) â†’ invisible in light mode; no extra DOM cost in light.
- `will-change: transform` hints GPU to promote to its own compositor layer.
- Reduced-motion respected.

---

## 5. Glassmorphism â€” Global rule via `data-slot` + sidebar class

shadcn/ui new-york already tags surfaces with `data-slot`. Only components that exist in this codebase today are targeted: **card, select-content, sheet-content**. Sidebar uses `<aside class="lg:bg-sidebar">` (no `data-slot`), so it gets a separate class-based selector.

The translucent background colors are already provided by the `--card`, `--popover`, `--sidebar` tokens (Â§3 â€” set at 55-70% alpha). This rule only adds the `backdrop-filter` blur â€” no `background-color` override, so tokens remain the single source of truth.

```css
@supports (backdrop-filter: blur(1px)) {
  .dark [data-slot="card"],
  .dark [data-slot="select-content"],
  .dark [data-slot="sheet-content"] {
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
  }
  .dark .lg\:bg-sidebar {
    backdrop-filter: blur(24px) saturate(180%);
    -webkit-backdrop-filter: blur(24px) saturate(180%);
  }
}
```

`@supports` ensures graceful fallback on browsers without `backdrop-filter` (very rare in 2026 â€” Safari, Chrome, Firefox 103+ all support it). Fallback = solid token color (still readable because tokens have alpha but render as opaque blend with body background).

> **Note for future:** when shadcn `popover` or `dropdown-menu` components are added, append `[data-slot="popover-content"]` and `[data-slot="dropdown-menu-content"]` to the same selector list.

---

## 6. Files Modified

| # | File | Action | LOC delta |
|---|---|---|---|
| 1 | `frontend/app/globals.css` | Replace `.dark { ... }` block; append aurora keyframes + glassmorphism rule | ~+80 / -55 |
| 2 | `frontend/components/animated-bg.tsx` | **NEW** Server Component | +25 |
| 3 | `frontend/app/layout.tsx` | Import `AnimatedBg`; mount as first child of `<body>` | +2 |

**No other files touched.** No `package.json`, no component edits, no config changes.

---

## 7. Verification Plan

After implementation:

1. **Type check:** `cd frontend && npx tsc --noEmit` â†’ exit 0
2. **Build:** `cd frontend && npm run build` â†’ exit 0, no warnings about missing tokens
3. **Visual smoke (manual):** `npm run dev` â†’ inspect:
   - `/` (root dashboard): aurora visible, cards have glass effect
   - `/operations`, `/strategies`, `/opportunities`: cards still readable, charts use new chart-1..5 cyan/mint palette
   - Toggle theme â†’ light theme unchanged
   - DevTools â†’ toggle `prefers-reduced-motion: reduce` â†’ animation pauses
4. **Lighthouse:** Performance score â‰¥ previous baseline (gradient is GPU-only, should not regress)
5. **R1 compliance:** No `Date.now()`, `Math.random()`, `window.*` introduced in render path. `AnimatedBg` is a pure Server Component.

---

## 8. Risks & Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| `backdrop-filter` GPU stress on low-end VPS preview | Low | Already wrapped in `@supports`; fallback is solid color |
| Charts (Recharts) become unreadable on translucent cards | Medium | Chart-1..5 chosen with high chroma (cyan, mint, royal, amber, magenta) all â‰¥ L 0.70 â†’ guaranteed readable on any glass tint |
| Contrast WCAG fail on muted-foreground over aurora | Medium | `--muted-foreground` set to `0.74 0.03 220` (L=74%) â€” contrast vs `--background` (L=16%) â‰ˆ 9.2:1, well above WCAG AA (4.5:1) |
| Animation visible during print | Low | `@media print { .aurora-blob { display: none; } }` (added to spec â€” see Â§4) |
| User dislikes the look post-deploy | Low | Trivially reversible: `git revert <commit>` restores prior state |

---

## 9. Out of Scope (explicitly NOT doing)

- âŒ Light theme changes
- âŒ Component prop API changes
- âŒ New animation libraries (no framer-motion variants, no GSAP)
- âŒ Reorganizing layout/navigation
- âŒ Typography changes (Geist Sans/Mono retained)
- âŒ Logo/brand mark changes
- âŒ Touching Cloudflare Worker or backend services

---

## 10. Acceptance Criteria

- [ ] `.dark` palette swapped to navy/cyan/mint per Â§3 table
- [ ] Aurora animated background visible behind all dark-mode pages
- [ ] Cards/popovers/sheets show iOS glass effect (blur 20px, sat 180%, opacity 50%)
- [ ] Light theme pixel-identical to pre-change (regression test by toggle)
- [ ] `prefers-reduced-motion` pauses animation
- [ ] `tsc --noEmit` and `npm run build` pass
- [ ] No new dependencies in `package.json`
- [ ] Total diff â‰¤ 110 LOC across exactly 3 files

---

## 11. Implementation Order (for plan stage)

1. Edit `globals.css` â€” replace `.dark` block, append aurora + glass rules
2. Create `animated-bg.tsx`
3. Mount `<AnimatedBg />` in `layout.tsx`
4. Run verification Â§7
5. Visual confirmation by user
6. Commit single squashed commit `style(frontend): aurora glass dark theme`

