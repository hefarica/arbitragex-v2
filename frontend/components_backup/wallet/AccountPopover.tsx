"use client";

/**
 * AccountPopover — a compact popover showing the connected account identity
 * (abbreviated address, chain, read-only balance) and a Disconnect action.
 * Disconnect is a local wallet action; it moves no funds and signs nothing.
 *
 * SSR-safe: renders a disconnected shell until mounted (avoids React #418).
 */

import { useEffect, useState } from "react";
import { useAccount, useDisconnect } from "wagmi";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useWalletBalances } from "@/hooks/useWalletBalances";
import { chainInfoSync } from "@/lib/chains";

function abbreviate(addr: string): string {
  return addr.length > 10 ? `${addr.slice(0, 6)}…${addr.slice(-4)}` : addr;
}

export function AccountPopover() {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const { address, isConnected, chainId } = useAccount();
  const { disconnect } = useDisconnect();
  const balance = useWalletBalances();

  if (!mounted || !isConnected || !address) {
    return (
      <Badge variant="secondary" data-testid="account-popover-disconnected">
        No account connected
      </Badge>
    );
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button type="button" variant="secondary" data-testid="account-popover-trigger">
          <span className="font-mono">{abbreviate(address)}</span>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-72 space-y-3" data-testid="account-popover-content">
        <div>
          <p className="text-xs uppercase tracking-widest text-muted-foreground/70">Address</p>
          <p className="mt-1 break-all font-mono text-xs">{address}</p>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-xs text-muted-foreground">Network</span>
          <Badge variant="outline">{chainInfoSync(chainId ?? 0).name}</Badge>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-xs text-muted-foreground">Balance (read-only)</span>
          <span className="font-mono text-xs">
            {balance.formatted ? `${balance.formatted} ${balance.symbol ?? ""}`.trim() : "—"}
          </span>
        </div>
        <Button type="button" variant="outline" size="sm" className="w-full" onClick={() => disconnect()}>
          Disconnect
        </Button>
      </PopoverContent>
    </Popover>
  );
}
