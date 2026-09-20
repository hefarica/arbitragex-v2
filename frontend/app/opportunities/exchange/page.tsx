import { redirect } from "next/navigation";

// CONSOLIDACIÓN (orden operador 2026-09-19): /opportunities es la página
// oficial ÚNICA de oportunidades. Los motores de esta página (atlas glass,
// ExchangeFilterBar, PriceTicker, badge modo, cap de memoria) fueron portados
// al cliente oficial. Esta ruta ahora redirige — nada se perdió.
export default function OpportunitiesExchangePage() {
  redirect("/opportunities");
}
