import WalletsClient, { type WalletsSnapshot } from "./WalletsClient";
import { getValidated } from "@/lib/frontier";
import { WalletsResponseSchema } from "@/lib/schemas";

export const metadata = { title: "Observers & Allowances | QuantumX" };
export const dynamic = "force-dynamic";

// OMEGA-8 / M5 Fase 9 (P1-WL-1): wallets SSR is now Zod-parsed via the
// frontier client. Invalid shape, 404 and 5xx surface as explicit `source`
// labels — no more `Array.isArray(data?.wallets) ? data.wallets : []`
// silently coercing every failure mode into "empty".
async function getInitialWallets(): Promise<WalletsSnapshot> {
  const r = await getValidated("/api/v1/wallets", WalletsResponseSchema);
  if (r.kind === "ok") {
    const wallets = r.data.wallets ?? r.data.items ?? r.data.rows ?? [];
    return { wallets: wallets as unknown as WalletsSnapshot["wallets"], source: "server-snapshot" };
  }
  if (r.kind === "endpoint_not_implemented") {
    return { wallets: [], source: "endpoint-not-implemented" };
  }
  if (r.kind === "invalid_response") {
    return { wallets: [], source: `invalid-response:${r.detail}` };
  }
  if (r.kind === "auth_required") {
    return { wallets: [], source: `auth-required:${r.status}` };
  }
  return { wallets: [], source: `server-fetch-failed:${r.detail}` };
}

export default async function WalletsPage() {
  const initialSnapshot = await getInitialWallets();
  return <WalletsClient initialSnapshot={initialSnapshot} />;
}
