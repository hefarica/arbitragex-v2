import QuantClient from "./QuantClient";
import { getApiBaseUrl } from "@/lib/api-client";
import { fetchQuantLayers } from "@/lib/quant-layers";

export const dynamic = "force-dynamic";

/**
 * /quant — el libro cuantitativo (hojas 05_EDGES → 09_DASHBOARD) como pantalla.
 *
 * Primer pintado por el Server Component (R1: snapshot serializable → estado
 * inicial del cliente), de modo que la tabla ya trae cifras sin esperar al poll.
 * El cliente refresca cada 15 s contra el EDGE público (misma URL, mismo
 * contrato: `fetchQuantLayers` es el único camino de lectura).
 *
 * Deep links:
 *   ?window_minutes=15|60|240|1440  → ventana de la capa (default 60)
 *   ?nocomputado=1                  → arranca mostrando las filas declaradas no
 *                                     computadas con su razón (default: OCULTAS,
 *                                     porque no llevan cifras)
 */
export default async function QuantPage({
  searchParams,
}: {
  searchParams?: {
    window_minutes?: string | string[];
    nocomputado?: string | string[];
  };
}) {
  const first = (v: string | string[] | undefined) => (Array.isArray(v) ? v[0] : v);

  const requested = Number(first(searchParams?.window_minutes) ?? 60);
  const windowMinutes = Number.isFinite(requested)
    ? Math.max(1, Math.min(24 * 60, Math.trunc(requested)))
    : 60;
  const showNoComputado = first(searchParams?.nocomputado) === "1";

  // Server-side: INTERNAL_EDGE_URL reaches the edge over the docker network
  // (same rule as the opportunities page — no Cloudflare, no rate-limit bucket).
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  const initialSnapshot = await fetchQuantLayers(EDGE_URL, {
    windowMinutes,
    limit: 500,
    top: 10,
  });

  return (
    <QuantClient
      initialSnapshot={initialSnapshot}
      initialWindowMinutes={windowMinutes}
      initialShowNoComputado={showNoComputado}
    />
  );
}
