# G-ROOT-ENTRYPOINTS-01 — Ambigüedad de entrypoints raíz (nuevo, cierra D-07)

- **Tipo:** gate anti-regresión estático.
- **Script:** `scripts/gates/g-root-entrypoints-01.sh` (exit 0 = PASS, exit 1 = FAIL).
- **Estado en HEAD `fe62cdce` + P3:** **PASS** (evidencia abajo, reproducible).

## Qué fija

1. **Cada `.md` raíz rastreado de 0 bytes está en la allowlist declarada** (doctrina *placeholder* conocida). Un documento raíz vacío **nuevo** falla el gate.
2. **La serie README está referenciada desde `README.md`**, así el punto de entrada es inequívoco sin renombrar archivos que se citan entre sí por nombre.
3. **Cada `*.bak` / `*.backup` rastreado está en la allowlist declarada** (G-BACKUP-01 ya los excluye de todo conteo; borrarlos es una decisión del propietario, no de este gate).

## Decisión de forma (por qué no se renombran los README)

`README-1..4.md` **no son entrypoints alternativos**: son una serie ordenada (resumen ejecutivo → metodología 13 pasos → guía de ejecución Playwright → reporting 40 secciones) y se citan entre sí por nombre (`README-2.md` cita `README-1.md`, `README-3.md`, `README-4.md`; `README-3.md` cita `README-1.md` y `README-2.md`). Renombrarlos rompería esas referencias internas y no aportaría nada. La ambigüedad se resuelve **en el entrypoint**: `README.md` ahora indexa la serie con su tema y su orden.

Los cuatro `.md` en 0 bytes son doctrina *placeholder* en español, intercalada con la serie inglesa que sí tiene contenido. **No se les inventa contenido** (R8): quedan declarados como deuda (D-09) y fijados por allowlist para que no crezcan en silencio.

## Evidencia (comando reproducible + salida real)

```
$ ./scripts/gates/g-root-entrypoints-01.sh
G-ROOT-ENTRYPOINTS-01 — root entrypoint ambiguity gate
  info  tracked 0-byte root .md: ['00_MANIFIESTO_DE_EFICIENCIA_ESTRUCTURAL.md', '01_RESOLUCION_ESTADO_ASIMETRICO.md', '02_INYECCION_ENTROPIA_TRANSITORIA.md', '03_EQUILIBRIO_MULTIVARIABLE_CIEGO.md']
  PASS  every 0-byte root .md is a declared placeholder (allowlist)
  info  tracked .bak/.backup: ['frontend/app/page.tsx.backup', 'package.json.bak']
  PASS  every stray backup file is declared (G-BACKUP-01 excludes it from counts)
  PASS  0-byte root .md and stray backups stay within their declared allowlists
  PASS  README-1..4.md are referenced from README.md (entry point unambiguous)

G-ROOT-ENTRYPOINTS-01: PASS
```
