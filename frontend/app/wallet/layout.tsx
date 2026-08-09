import type { ReactNode } from "react";
import { headers } from "next/headers";

import { Web3Provider } from "@/app/providers/Web3Provider";

// B-01: Web3Provider (wagmi + react-query + RainbowKit) mounts ONLY on the
// /wallet route — the sole surface that connects a browser wallet. Previously
// it wrapped the whole app from the root layout, loading WalletConnect on all
// 56 pages (hydration lag + walletconnect.org calls). It exposes read-only
// connectivity only: no signer, no capital, no broadcast.
//
// SSR: forward the request Cookie to wagmi's cookieToInitialState so server and
// client agree on the initial connection state (avoids React #418).
export const dynamic = "force-dynamic";

export default async function WalletLayout({ children }: { children: ReactNode }) {
  const cookie = (await headers()).get("cookie");
  return <Web3Provider cookie={cookie}>{children}</Web3Provider>;
}
