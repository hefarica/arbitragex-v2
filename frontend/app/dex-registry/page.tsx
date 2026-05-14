import DexRegistryClient, { type DexRegistrySnapshot } from "./DexRegistryClient";
import { getApiBaseUrl } from "@/lib/api-client";

export const metadata = { title: "Exchange Registry | QuantumX" };
export const dynamic = "force-dynamic";

async function getInitialDexes(): Promise<DexRegistrySnapshot> {
  const EDGE_URL =
    process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  try {
    const res = await fetch(`${EDGE_URL}/api/v1/dexes`, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (res.status === 404) {
      return { dexes: [], source: "endpoint-not-implemented" };
    }
    if (!res.ok) {
      return { dexes: [], source: `server-fetch-failed:${res.status}` };
    }
    const data = await res.json();
    const dexes = Array.isArray(data?.dexes) ? data.dexes : [];
    return { dexes, source: "server-snapshot" };
  } catch {
    return { dexes: [], source: "server-fetch-failed" };
  }
}

export default async function DexRegistryPage() {
  const initialSnapshot = await getInitialDexes();
  return <DexRegistryClient initialSnapshot={initialSnapshot} />;
}
