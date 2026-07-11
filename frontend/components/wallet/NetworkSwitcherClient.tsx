"use client";

import dynamic from "next/dynamic";

const NetworkSwitcherDynamic = dynamic(
  () => import("@/components/wallet/NetworkSwitcher").then((m) => m.NetworkSwitcher),
  { ssr: false }
);

export function NetworkSwitcherClient() {
  return <NetworkSwitcherDynamic />;
}
