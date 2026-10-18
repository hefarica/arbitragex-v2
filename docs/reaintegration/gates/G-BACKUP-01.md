# G-BACKUP-01 — Exclusión de árboles backup/recovery (nuevo, cierra D-01)

- **Tipo:** gate anti-regresión estático (obligatorio antes de cualquier P5).
- **Script:** `scripts/gates/g-backup-01.sh` (exit 0 = PASS, exit 1 = FAIL).
- **Estado en HEAD `fe62cdce`:** **PASS** (evidencia abajo, reproducible).

## Qué fija

1. **Predicado de exclusión canónico, declarado una sola vez:**
   una ruta es *backup-class* si algún segmento de directorio es `_recovery` o termina en `_backup`, **o** el basename termina en `.backup` / `.bak`.
   Archivos cuyo *nombre* contiene "recovery"/"backup" pero no son copias (p.ej. `backend/sim-ctl/tests/simwire02_pel_recovery.rs`) **no** son backup-class.
2. **Ningún árbol backup es alcanzable desde build/typecheck:** `frontend/tsconfig.json` include/exclude, `frontend/next.config.js`, `workspaces` de `package.json` y `frontend/package.json`.
3. **Conteo de rutas sobre el set filtrado** (59 `page.tsx` en `frontend/app` a HEAD `fe62cdce`), estable en dos cómputos independientes.
4. **Polución repo-wide medida** (el síntoma de D-01): `find . -name page.tsx` = **111**; backup-class = **52**; filtrado = **59**.
5. **Backup-class presentes** ⇒ el gate tiene dientes (si no hubiera ninguna, el gate sería decorativo).

## Evidencia (comando reproducible + salida real)

```
$ ./scripts/gates/g-backup-01.sh
G-BACKUP-01 — backup/recovery exclusion gate
repo root: /home/user/repos/arbitragex-v2

1) backup-class paths tracked in git: 304
  PASS  frontend/tsconfig.json include/exclude has no backup path
  PASS  frontend/next.config.js references no backup path
  PASS  package.json workspaces reference no backup path

3) live App Router page.tsx (filtered): 59  [all=59, backup-class dropped=0] (cross-check: 59)
  PASS  filtered route count is stable across two computations
  PASS  live page.tsx count == 59 (HEAD fe62cdce)
  PASS  legitimate source simwire02_pel_recovery.rs is NOT backup-class

5) repo-wide page.tsx: all=111, backup-class=52, filtered=59
  PASS  backup-class page.tsx ARE present repo-wide (drift is real, gate has teeth)
  PASS  repo-wide filtered page.tsx count == 59

G-BACKUP-01: PASS
```

- Lista completa de rutas backup-class rastreadas por git: `docs/reaintegration/evidence/G-BACKUP-01.tracked.txt` (304 rutas).
- Árboles: `_recovery/`, `frontend/_recovery/`, `frontend/app_backup/`, `frontend/components_backup/`, `frontend/features_backup/`.
- Archivos sueltos: `frontend/app/page.tsx.backup`, `package.json.bak`.

## Cómo se aplica en cada fase

- **P0 (inventario):** todo conteo de rutas/páginas usa el set filtrado.
- **P3/P4 (build):** los manifiestos no deben referenciar backup-class (checks 2).
- **P5 (E2E):** el barrido de rutas navegables parte del set filtrado; un `page.tsx` de backup **nunca** cuenta como ruta.

## Deuda declarada

- El gate es **estático**. No se ejecutó `next build` ni Playwright (sin `node_modules`/toolchain en este sandbox) — ver `FINAL-CLOSURE.md`.
- Los árboles backup **siguen en el repo** (no se borran: doctrina aditiva). El gate los **excluye**, no los elimina. Su retirada es una decisión aparte de Hector.
