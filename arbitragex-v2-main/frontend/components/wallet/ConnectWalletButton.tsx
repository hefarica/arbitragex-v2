"use client";

/**
 * ConnectWalletButton — the connect/disconnect affordance for the read-only
 * wallet surface. Built on RainbowKit's headless ConnectButton.Custom so we
 * control the SSR shell: on the server (and before mount) we render a stable
 * disconnected button; after hydration the live wallet state fills in. This
 * avoids React #418 hydration mismatches.
 *
 * SECURITY: connecting only exposes the wallet address read-only. There is no
 * signer, no broadcast. The WalletConnect option degrades to a fail-honest note
 * when NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID is absent (injected/MetaMask still
 * works via EIP-6963).
 */

import { useEffect, useState } from "react";
import { ConnectButton } from "@rainbow-me/rainbowkit";
import { WalletIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { describeConnectors } from "@/lib/web3/connectors";

export function ConnectWalletButton() {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const connectors = describeConnectors();
  const wc = connectors.find((c) => c.id === "walletConnect");

  // Stable disconnected shell for SSR + pre-mount. data-testid lets E2E assert
  // the button exists without depending on wallet state.
  if (!mounted) {
    return (
      <div className="flex flex-col gap-2" data-testid="connect-wallet-button">
        <Button type="button" variant="default" disabled>
          <WalletIcon className="size-4" />
          Connect wallet
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2" data-testid="connect-wallet-button">
      <ConnectButton.Custom>
        {({ account, chain, openAccountModal, openChainModal, openConnectModal, mounted: rkMounted }) => {
          const ready = rkMounted;
          const connected = ready && account && chain;
          return (
            <div
              {...(!ready ? { "aria-hidden": true, style: { opacity: 0, pointerEvents: "none", userSelect: "none" } } : {})}
            >
              {!connected ? (
                <Button type="button" variant="default" onClick={openConnectModal} data-testid="connect-open">
                  <WalletIcon className="size-4" />
                  Connect wallet
                </Button>
              ) : chain.unsupported ? (
                <Button type="button" variant="destructive" onClick={openChainModal} data-testid="unsupported-chain-button">
                  Unsupported chain — switch
                </Button>
              ) : (
                <div className="flex items-center gap-2">
                  <Button type="button" variant="outline" onClick={openChainModal} data-testid="chain-button">
                    {chain.name ?? `Chain ${chain.id}`}
                  </Button>
                  <Button type="button" variant="secondary" onClick={openAccountModal} data-testid="account-button">
                    {account.displayName}
                  </Button>
                </div>
              )}
            </div>
          );
        }}
      </ConnectButton.Custom>

      {wc && !wc.available && (
        <Badge variant="warning" data-testid="walletconnect-degraded">
          WalletConnect unavailable: {wc.reason}
        </Badge>
      )}
    </div>
  );
}
