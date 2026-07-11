"use client";

import dynamic from "next/dynamic";

const AccountPopoverDynamic = dynamic(
  () => import("@/components/wallet/AccountPopover").then((m) => m.AccountPopover),
  { ssr: false }
);

export function AccountPopoverClient() {
  return <AccountPopoverDynamic />;
}
