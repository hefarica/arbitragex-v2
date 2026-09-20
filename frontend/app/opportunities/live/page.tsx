import { redirect } from "next/navigation";

// CONSOLIDACIÓN (orden operador 2026-09-19): /opportunities/live era la URL
// legacy de la página live — ahora TODO llega a la página oficial.
export default function OpportunitiesLivePage() {
  redirect("/opportunities");
}
