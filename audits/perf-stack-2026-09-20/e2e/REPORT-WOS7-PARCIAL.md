# E2E WO-S7 (fase parcial #620) — Veredicto del browser-verifier

Fecha: 2026-09-20 · Sesión: -89 · Agente: dapp-browser-verifier (a45f5067)
Target: https://arbx.ape-tv.net/opportunities · Deploy verificado: 44f64a74 (#620 CARDS-DEDUP-HOPS, edge+frontend rebuild --no-cache)
Presupuesto consumido: 1/5 cargas de página (+ interacciones in-app). Ventana: 14:54:49→14:58:21 hora de página.

## Veredicto global: PASS (deploy #620 verificado en dominio público)

| Ítem | Veredicto |
|---|---|
| Dedup 1-card-por-ruta | **PASS** — 2 cards = 2 rutas distintas, 0 duplicados. Ticker crudo ("20 recent opportunities") muestra las mismas rutas repetidas decenas de veces (WETH/USDT ~40+, DAI/WETH ~6) pero el render colapsa a 1 card por route group. Force refresh mantuvo conteo=2. |
| Badge confirmaciones | **PASS** — ×6 y ×44 con tooltip "Ruta re-detectada N veces en la ventana" + first_seen/last_seen relativos ("1ª 5m"/"✓ 4m") avanzando en vivo. |
| Filtro hops | **GAPS (tooling)** — control existe (combobox "Filter by hop count", "All hops"/"2 hops"); `<select>` nativo no operable por el toolset MCP. Discriminante débil de todos modos (sólo rutas 2-hop en ventana). Verificación humana de 30s o Playwright nativo sugerida. |
| WS vivo | **PASS parcial** — socket.io establecido (sid 3x32CYLq…), panel routes/pairs LIVE, PRICES LIVE con `upd` avanzando. Contadores ×6/×44 no incrementaron (~2.5 min): rutas stale sin re-detecciones nuevas — limitación del dato, no del dedup. |
| R8/RULE 00 | **PASS** — "no computado: non_positive_profit (R8)" con razón; ROI/prices "—"; cero ceros fabricados; sin banner QUARANTINED. |
| Console | **PASS (known)** — 27 errores idénticos = 503 /api/quote/anchor (quote_anchor_not_published, key arbx:quote:anchor:1 ausente) = esperado pre-deploy-completo (#617/searcher-rs/api-server pendientes). Sin JS/WS/429/401. |

## Cards observadas (conteo 2, header coherente "0 viable / 2 total (2 rejected)")

- Card A: DAI 0x6b17…1d0f → WETH 0xc02a…6cc2 · UniV2→Sushi · dex_arb · hops=2 · REJECTED non_positive_profit · Gross $63.10 · ×6
- Card B: WETH → USDT 0xdac1…1ec7 · UniV2→Sushi · dex_arb · hops=2 · REJECTED non_positive_profit · Gross $23.75 · ×44

## Anomalías para follow-up (ninguna bloquea #620)

1. **socket.io sin upgrade a websocket visible** — todo el tráfico observado `transport=polling` same-origin (/socket.io). Verificar server-side contra api-server:8080 (RULE 02: WS directo, jamás vía edge). Posible causa: proxy/túnel sin upgrade header o cliente sin upgrade. ANOMALÍA ABIERTA.
2. **quote_anchor 503 persistente** (arbx:quote:anchor:1 ausente) — se espera que resuelva con el deploy completo WO-S7 (searcher-rs/api-server). Mientras, chip socket en ERROR (honesto: runtime_ack admin-gated by-design + quote_anchor 503) y precios "—".
3. **Filtro hops no verificable por toolset** — control presente; falta verificación interactiva.

## Capturas

- 01-initial-cards.png (estado inicial 14:55, full-page)
- 02-hops-filter-control.png (combobox hops)
- 03-post-refresh-dedup-stable.png (post Force refresh, conteo estable 2)
