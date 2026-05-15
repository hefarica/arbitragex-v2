import DexRegistryClient, { type DexRegistrySnapshot } from "./DexRegistryClient";
import { getValidated } from "@/lib/frontier";
import { DexRegistryResponseSchema } from "@/lib/schemas";

export const metadata = { title: "Exchange Registry | QuantumX" };
export const dynamic = "force-dynamic";

// OMEGA-8 / M5 Fase 9 (P1-DR-1): dex-registry SSR is now Zod-parsed via the
// frontier client — invalid shape no longer silently becomes `dexes: []`.
async function getInitialDexes(): Promise<DexRegistrySnapshot> {
  const r = await getValidated("/api/v1/dexes", DexRegistryResponseSchema);
  if (r.kind === "ok") {
    const dexes = r.data.dexes ?? r.data.items ?? r.data.rows ?? [];
    return { dexes: dexes as unknown as DexRegistrySnapshot["dexes"], source: "server-snapshot" };
  }
  if (r.kind === "endpoint_not_implemented") {
    return { dexes: [], source: "endpoint-not-implemented" };
  }
  if (r.kind === "invalid_response") {
    return { dexes: [], source: `invalid-response:${r.detail}` };
  }
  if (r.kind === "auth_required") {
    return { dexes: [], source: `auth-required:${r.status}` };
  }
  return { dexes: [], source: `server-fetch-failed:${r.detail}` };
}

export default async function DexRegistryPage() {
  const initialSnapshot = await getInitialDexes();
  return <DexRegistryClient initialSnapshot={initialSnapshot} />;
}
