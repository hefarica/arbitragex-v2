"use client";

/**
 * ConnectWalletButtonClient — Client Component wrapper that dynamically imports
 * ConnectWalletButton with SSR disabled. This prevents React context errors
 * during static page generation while allowing the parent page to remain a
 * Server Component.
 */

import dynamic from "next/dynamic";
import { Button } from "@/components/ui/button";
import { WalletIcon } from "lucide-react";

const ConnectWalletButtonDynamic = dynamic(
  () => import("@/components/wallet/ConnectWalletButton").then((m) => m.ConnectWalletButton),
  {
    ssr: false,
    loading: () => (
      <Button type="button" variant="default" disabled>
        <WalletIcon className="size-4" />
        Connect wallet
      </Button>
    ),
  }
);

export function ConnectWalletButtonClient() {
  return <ConnectWalletButtonDynamic />;
}
