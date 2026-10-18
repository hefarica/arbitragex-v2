# ADR-002 — `/admin` raíz resuelve 404 (drift D-02, gate G-ADMIN-ROOT-01)

- **Estado:** ACEPTADO (decisión de P0, no toca código)
- **Fecha:** 2026-10-18
- **Contexto:** `arbitragex-v2` @ `fe62cdce` (main)
- **Gate:** `scripts/gates/g-admin-root-01.sh`

## 1. Problema

`frontend/app/admin/` **no tiene `page.tsx`**: sólo `signin/`, `chains/`, `topology/` y `__tests__/`. En el App Router de Next.js, `/admin` (raíz) devuelve **404**. El plan pedía "decidir `/admin` raíz hoy 404".

## 2. Evidencia (HEAD `fe62cdce`)

- Rutas admin existentes: `/admin/signin`, `/admin/chains`, `/admin/topology`.
- `frontend/components/nav-items.ts` referencia **exactamente esas tres** y **ninguna apunta a `/admin` a secas** (verificado: 0 literales `"/admin"` en `frontend/components`, `frontend/features`, `frontend/app`).
- Cada sub-ruta admin tiene su propia guarda: `/admin/signin` establece la cookie httpOnly `arbx_admin_session` (contrato V-AT-1); `chains`/`topology` son operativas.

## 3. Decisión

**Se mantiene el 404 en `/admin` raíz.**

Razón: no existe consumidor. Añadir `app/admin/page.tsx` crearía una ruta navegable nueva **sin consumidor real**, violando el invariante "ningún campo/ruta nuevo en wire sin consumidor real". Además `/admin` es un prefijo, no un destino: el operador llega por el sidebar a `/admin/signin`.

## 4. Consecuencia

- **No** se añade página (cero cambio en `frontend/`).
- El gate `G-ADMIN-ROOT-01` pina la decisión de forma que una futura contradicción falla en CI:
  1. `frontend/app/admin/page.tsx` NO existe;
  2. ningún `href`/literal apunta a `/admin` a secas;
  3. toda ruta `/admin/*` existente está presente en `nav-items.ts`.
- **Deuda declarada:** si Hector quiere una landing de admin, es una decisión de producto nueva (ruta + entrada de nav + consumidor) y abre su propio ADR; el 404 actual no es un bug.

## 5. Revert

Trivial: si se decide lo contrario, se crea `frontend/app/admin/page.tsx` (redirect SSR a `/admin/signin`) y se actualiza el gate en un paso.
