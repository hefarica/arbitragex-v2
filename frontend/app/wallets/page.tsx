import WalletsClient, { type WalletsSnapshot } from "./WalletsClient";
import { getApiBaseUrl } from "@/lib/api-client";

export const metadata = { title: "Wallets & Allowances | ArbitrageX" };
export const dynamic = "force-dynamic";

async function getInitialWallets(): Promise<WalletsSnapshot> {
  const EDGE_URL =
    process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  try {
    const res = await fetch(`${EDGE_URL}/api/v1/wallets`, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (res.status === 404) {
      return { wallets: [], source: "endpoint-not-implemented" };
    }
    if (!res.ok) {
      return { wallets: [], source: `server-fetch-failed:${res.status}` };
    }
    const data = await res.json();
    const wallets = Array.isArray(data?.wallets) ? data.wallets : [];
    return { wallets, source: "server-snapshot" };
  } catch {
    return { wallets: [], source: "server-fetch-failed" };
  }
}

export default async function WalletsPage() {
  const initialSnapshot = await getInitialWallets();
  return <WalletsClient initialSnapshot={initialSnapshot} />;
}
