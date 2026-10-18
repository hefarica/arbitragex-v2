# G-OMEGA-BASE-01 — Validez del pin `.omega-base-sha` (nuevo, cierra D-05)

- **Tipo:** gate anti-regresión estático (obligatorio antes de cualquier P5).
- **Script:** `scripts/gates/g-omega-base-01.sh` (exit 0 = PASS, exit 1 = FAIL).
- **Estado en HEAD `fe62cdce` + P3:** **PASS** (evidencia abajo, reproducible).

## Qué fija

1. El pin es un id de commit de **40 hex**.
2. El commit pinneado **existe como objeto** en el clon.
3. El pin **es igual a la base auditada del programa** (`docs/reaintegration/00-INVENTORY.json` → `repo.head`), que es la única fuente de verdad. Así el pin no puede volver a separarse en silencio de la base auditada.

## Corrección de método (por qué el hallazgo original estaba mal caracterizado)

El hallazgo D-04/D-05 original decía que `70db8183…` "no existe en el historial (`git cat-file` = absent)". Verificado contra la API de GitHub: el objeto **sí existe** (commiteado 2026-05-17), pero **no tiene ancestro común con `fe62cdce`** — el compare de GitHub devuelve 404 en ambos sentidos, es decir, la historia de `main` fue **reescrita** y el commit quedó huérfano. Además, `git cat-file -t` sobre un clon *shallow* o *single-branch* devuelve exactamente ese "Not a valid object name": el diagnóstico original era un artefacto del método de clonado.

Cambia el arreglo: no es "el commit no existe" (habría que inventar otro), es "el pin apunta a una historia que ya no es la viva".

## Evidencia (comando reproducible + salida real)

```
$ ./scripts/gates/g-omega-base-01.sh
G-OMEGA-BASE-01 — .omega-base-sha pin validity gate
  info  pinned BASE_SHA = fe62cdceb6dc0b18afed8db03b7dc7ab64e38d6b
  PASS  pin is a 40-hex commit id
  PASS  pinned commit is present as an object in this clone
  info  audited programme base (inventory.repo.head) = fe62cdceb6dc0b18afed8db03b7dc7ab64e38d6b
  PASS  pin equals the audited programme base

G-OMEGA-BASE-01: PASS
```
