# 5d — Inventario de consolidación /opportunities (2026-09-20)

> Investigación read-only (agente Explore, 22 tool calls). Cierra la fase de
> inventario del ítem 5d del CHECKLIST. La migración pendiente queda enumerada abajo.

## Hallazgo clave

La absorción YA está aplicada: `/opportunities/exchange`, `/live` y `/by-strategy`
son redirects 307 a `/opportunities`
(`frontend/app/opportunities/exchange/page.tsx:8`, `live/page.tsx:6`, `by-strategy/page.tsx:8`).
El cliente de exchange (`OpportunitiesExchangeClient.tsx`) fue eliminado (ni en `app_backup/`).
El inventario relevante = qué quedó portado y qué quedó huérfano.

## Portado a la página oficial (OK)

| Feature | Evidencia |
|---|---|
| Filtros familia/cadena/búsqueda/min-yield (`applyExchangeFilters`) | `OpportunitiesClient.tsx:319-334` ← `components/opportunities/exchange/ExchangeFilterBar.tsx:197` |
| PriceTicker G-PRICE-1 (WS `usePricesStream`) | `OpportunitiesClient.tsx:524` (`PriceTicker.tsx:39`) |
| Badge TERMINUS paper/live (`usePaperModeState`) | `OpportunitiesClient.tsx:112-119,511-517` |
| Cap de memoria VISIBLE_CAP=60 + "show more" | `OpportunitiesClient.tsx:72,110,606-621` |

## NO portado (huérfano — migrar o decidir view-mode)

1. **`OpportunityExchangeCard.tsx`** — card atlas glass: dos caras eval/diag
   ("Por qué NO pasó"), kv-ledger neón, badge QuantumX, caja de evidence
   shadow-sim (147, 205, 499-523), EXECUTE, diagnostic de detección hueca
   (154-166). Test vivo: `__tests__/OpportunityExchangeCard.test.tsx`.
   La página oficial usa `OpportunityTradeCard` (visual distinto, sin cara diag).
2. **`atlas-glass.css`** (549 líneas, design language `docs/atlas_264.html`) — sin importer.
3. **JSX de `ExchangeFilterBar`** — solo se importan tipos/filtro; los chips led,
   catálogo `/api/chains` (`useChains`) y búsqueda "cartridge id" NO están montados.
4. **`OpportunitiesByStrategyClient.tsx`** — agrupación por estrategia (poll 4s);
   retenido intencionalmente como futuro view-mode (`by-strategy/page.tsx:4-6`).

## Divergencias del port (verificar/migrar)

- Familias semilla: oficial `["triangular","cross_chain","liquidation","flashloan_arb"]`
  (`OpportunitiesClient.tsx:327`) vs `BASE_STRATEGIES` de exchange
  (incluye `dex_arb`, `backrun`; `ExchangeFilterBar.tsx:80`).
- Selector de cadena: oficial deriva del feed con conteo (`OpportunitiesClient.tsx:340-347`);
  exchange usaba catálogo `/api/chains` con nombres.
- **Bug latente de búsqueda**: placeholder "token / dex / strategy"
  (`OpportunitiesClient.tsx:492`) pero `applyExchangeFilters` solo matchea
  `strategy_kind` (`ExchangeFilterBar.tsx:209`).

## Endpoints (ambas rutas consumen lo mismo)

REST `/api/opportunities/live` (SSR + refresh), WS `useOmniOpportunities` (omni-store),
`POST /api/v1/opportunities/:id/simulate` (shadow sim), `usePricesStream`,
`usePaperModeState`, `getTradingConfig`.

## Riesgos de borrado (referencias residuales)

- `.github/workflows/auto-deploy-vps.yml:399-400` — smoke post-deploy aún curlea
  `/opportunities/exchange` (el 307 pasa `curl -sf` pero ya no verifica contenido;
  actualizar a `/opportunities`).
- `frontend/e2e/new-pages.spec.ts:84-94` — ya testea redirect de by-strategy
  (patrón a replicar si se borra exchange).
- Tests vivos sobre huérfanos: `OpportunityExchangeCard.test.tsx`,
  `applyExchangeFilters.test.ts`, `OpportunitiesByStrategyClient.projection.test.tsx`.
- Nav actual NO enlaza exchange; sin referencias en sitemap/middleware.
- Backups pre-consolidación: `app_backup/opportunities/`, `features_backup/opportunities/`.

## Plan de migración propuesto (pendiente gate operador para PRs de frontend)

- M1: montar `OpportunityExchangeCard` como cara diag / view-mode en la oficial
  (nada se tira: el visual atlas pasa a ser alternable).
- M2: montar JSX `ExchangeFilterBar` (chips + catálogo `/api/chains` + búsqueda
  cartridge id) y unificar `BASE_STRATEGIES` (agregar `dex_arb`, `backrun`).
- M3: importar `atlas-glass.css` donde se use la card atlas.
- M4: fix bug búsqueda (matchear token/dex además de strategy_kind).
- M5: actualizar smoke `auto-deploy-vps.yml:399-400` a `/opportunities`.
- M6: `OpportunitiesByStrategyClient` como view-mode "por estrategia" en la oficial.
