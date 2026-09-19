# WO-CARDS-COMPLETE-01 — Cards con datos completos SIEMPRE (orden operador 2026-09-17)
> Evidencia: screenshot operador (3 cards UniswapV2→SushiSwap 2-hop REJECTED con esqueletos)
> + GAP-2 del gang E2E. Principio: NO fabricar (RULE 00) — COMPUTAR SIEMPRE.

## Defecto
Las oportunidades rechazadas se emiten SIN los campos que las cards muestran:
el sizing nunca corre ("Inverse-sizing not run"), el detector no se emite
("no emitido" — gap conocido route_metadata), no hay probe-quote economics,
no hay target. El usuario ve esqueletos.

## Spec campo-por-campo (TODA oportunidad emitida, aceptada O rechazada, lleva):
1. **amount_in / BPS / probe-economics**: quote al probe estándar SIEMPRE
   (aunque el sizing óptimo no corra) → IN/BPS/GROSS-Out con números reales.
2. **NET y SIM**: si no llegó a sim → valor del gate que la mató + economics
   al probe size (net = gross_probe − gas_estimate − fees_known). Label de
   SIM honesto: "not_simulated:<reason>" en vez de "—".
3. **DETECTOR**: route_metadata SIEMPRE emitido (cerrar el gap conocido
   "emit_rejected sin route" del ledger).
4. **LATENCIA**: stage timings del pipeline ya medidos (lat_candidates existe)
   → wire a la card.
5. **TARGET**: el sizing óptimo corre SIEMPRE que haya reserves (aunque el
   EV final sea negativo) → target real; solo "none" si faltan datos crudos
   (con razón exacta).
6. **Cost breakdown**: gas_estimate (gas oracle ya vive), TLS fee (leído
   on-chain, no asumido), DEX/LP fees del catálogo — al probe size mínimo.
7. **Frontend**: cero "—" sin explicación — cada campo vacío muestra
   "not computed: <razón>" (R8 visible), y los computables jamás están vacíos.

## Gates
- Test E2E: opportunity rechazada cualquiera → TODOS los campos != null en
  API y card (o razón exacta).
- Regresión: las aceptadas no cambian de semántica (net/sim como hoy).
- Un PR = un ID; builder + verificador (doctrina §12).
