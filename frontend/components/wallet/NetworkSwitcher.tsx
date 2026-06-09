"use client";

/**
 * NetworkSwitcher — lets the operator switch the connected wallet to one of the
 * operator-configured supported chains, and surfaces an explicit
 * "Unsupported chain" state when the wallet is on a chain outside the set.
 *
 * Switching networks is a wallet-side action (wallet_switchEthereumChain); it
 * moves no funds and signs no transaction. SSR-safe via the mounted gate.
 */

import { useEffect, useState } from "react";
import { useAccount, useChainId, useSwitchChain } from "wagmi";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { resolveSupportedChains, isSupportedChain } from "@/lib/web3/chains";

export function NetworkSwitcher() {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const { isConnected } = useAccount();
  const chainId = useChainId();
  const { chains, switchChain, isPending } = useSwitchChain();
  const supported = resolveSupportedChains();

  if (!mounted || !isConnected) {
    return (
      <div className="flex flex-wrap items-center gap-2" data-testid="network-switcher">
        <span className="text-sm text-muted-foreground">Connect a wallet to switch networks.</span>
      </div>
    );
  }

  const onUnsupported = !isSupportedChain(chainId);

  return (
    <div className="flex flex-wrap items-center gap-2" data-testid="network-switcher">
      {onUnsupported && (
        <Badge variant="destructive" data-testid="network-unsupported">Unsupported chain</Badge>
      )}
      {supported.map((c) => {
        const active = c.id === chainId;
        const available = chains.some((wc) => wc.id === c.id);
        return (
          <Button
            key={c.id}
            type="button"
            size="sm"
            variant={active ? "default" : "outline"}
            disabled={active || isPending || !available || !switchChain}
            onClick={() => switchChain?.({ chainId: c.id })}
          >
            {c.name}
          </Button>
        );
      })}
    </div>
  );
}
