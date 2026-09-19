import { redirect } from "next/navigation";

// CONSOLIDACIÓN (orden operador 2026-09-19): /opportunities es la página
// oficial ÚNICA. La proyección por estrategia (features/opportunities/
// OpportunitiesByStrategyClient + by-strategy-grouping) permanece en el repo
// y puede volver como modo de vista sobre la página oficial — no se borró.
export default function OpportunitiesByStrategyPage() {
  redirect("/opportunities");
}
