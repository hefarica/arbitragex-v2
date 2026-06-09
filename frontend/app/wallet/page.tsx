/**
 * /wallet — the safe-gated, READ-ONLY institutional wallet surface.
 *
 * This page lets the operator CONNECT a browser wallet to expose their address
 * read-only, view balance, switch networks, authenticate via SIWE (identity
 * only), and record transaction INTENTS that always terminate at
 * BROADCAST_DISABLED. It NEVER exposes an enabled execute/broadcast/send path.
 *
 * R1 (Mounted Snapshot Pattern): this is a pure Server Component shell; every
 * interactive, wallet-derived, non-deterministic value lives inside the client
 * components below (all marked "use client"), each gated by a mounted flag. The
 * WalletSafetyBanner is always visible at the top.
 */

import { PageHeader } from "@/components/page-header";
import { WalletSafetyBanner } from "@/components/wallet/WalletSafetyBanner";
import { ConnectWalletButton } from "@/components/wallet/ConnectWalletButton";
import { WalletStatusCard } from "@/components/wallet/WalletStatusCard";
import { NetworkSwitcher } from "@/components/wallet/NetworkSwitcher";
import { AccountPopover } from "@/components/wallet/AccountPopover";
import { WalletIntentPanel } from "@/components/wallet/WalletIntentPanel";
import { SiweAuthPanel } from "@/app/wallet/SiweAuthPanel";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default function WalletPage() {
  return (
    <>
      <PageHeader
        title="Wallet (read-only)"
        lede="Connect a browser wallet to expose your address read-only, authenticate via Sign-In With Ethereum (identity only), and record transaction intents. No keys are held server-side; nothing is ever broadcast."
        actions={<ConnectWalletButton />}
      />

      <div className="space-y-6">
        <WalletSafetyBanner />

        <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
          <WalletStatusCard />

          <Card>
            <CardHeader>
              <CardTitle>Account & network</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <AccountPopover />
              <NetworkSwitcher />
            </CardContent>
          </Card>
        </div>

        <SiweAuthPanel />

        <WalletIntentPanel />
      </div>
    </>
  );
}
