# G-ADMIN-ROOT-01 — `/admin` raíz (nuevo, cierra D-02)

- **Tipo:** gate de consistencia de decisión (estático).
- **Script:** `scripts/gates/g-admin-root-01.sh` (exit 0 = PASS).
- **Decisión que pina:** ADR-002 — `/admin` raíz **permanece 404** (sin consumidor).
- **Estado en HEAD `fe62cdce`:** **PASS**.

## Qué fija

1. `frontend/app/admin/page.tsx` **no existe** (⇒ `/admin` = 404, por diseño).
2. Ningún `href`/literal de ruta apunta a `/admin` a secas (no hay consumidor insatisfecho).
3. Toda ruta `/admin/*` existente (`/admin/signin`, `/admin/chains`, `/admin/topology`) está presente en `frontend/components/nav-items.ts` (nada navegable queda huérfano).

## Evidencia (comando reproducible + salida real)

```
$ ./scripts/gates/g-admin-root-01.sh
G-ADMIN-ROOT-01 — /admin root resolution gate
  PASS  frontend/app/admin/page.tsx does NOT exist (bare /admin = 404)
  info  existing admin routes: frontend/app/admin/chains/page.tsx frontend/app/admin/signin/page.tsx frontend/app/admin/topology/page.tsx 
  PASS  no href/route literal points at bare /admin
  PASS  route /admin/signin is present in nav-items.ts
  PASS  route /admin/chains is present in nav-items.ts
  PASS  route /admin/topology is present in nav-items.ts

G-ADMIN-ROOT-01: PASS
```

## Por qué no es un FAIL de producto

El plan de P5 exige "página que no pinta en el primer render SSR = FAIL". `/admin` no es una página: es un prefijo de ruta sin entrada de navegación. Las tres páginas reales pintan por SSR y cada una tiene su guarda. La decisión está razonada en `adr/ADR-002-admin-root-404.md`.
