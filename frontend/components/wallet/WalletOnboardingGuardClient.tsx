"use client";

import dynamic from "next/dynamic";

const WalletOnboardingGuardDynamic = dynamic(
  () => import("@/components/wallet/WalletOnboardingGuard").then((m) => m.WalletOnboardingGuard),
  { ssr: false }
);

export function WalletOnboardingGuardClient() {
  return <WalletOnboardingGuardDynamic />;
}
