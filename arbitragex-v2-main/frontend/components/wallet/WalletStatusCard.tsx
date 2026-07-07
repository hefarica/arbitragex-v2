"use client";

/**
 * WalletStatusCard — connection + chain + balance summary for the connected
 * account, plus the hard safety badges (Read-only / Paper / Live disabled /
 * Capital exposure 0 / Broadcast disabled). SSR-safe: all wallet-derived live
 * values are gated behind the `mounted` flag from useWalletStatus, so the server
 * renders a stable disconnected shell.
 *
 * NEVER renders an enabled execute/broadcast/send affordance.
 */

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useWalletStatus } from "@/hooks/useWalletStatus";
import { useWalletBalances } from "@/hooks/useWalletBalances";
import { chainInfoSync } from "@/lib/chains";

function abbreviate(addr: string): string {
  return addr.length > 10 ? `${addr.slice(0, 6)}…${addr.slice(-4)}` : addr;
}

export function WalletStatusCard() {
  const { mounted, isConnected, address, chainId, supportedChain, runtime, runtimeError } = useWalletStatus();
  const balance = useWalletBalances();

  return (
    <Card data-testid="wallet-status-card">
      <CardHeader>
        <CardTitle className="flex items-center justify-between">
          <span>Wallet status</span>
          {/* suppressHydrationWarning on the individual status span — its text
              flips from "Read-only" (SSR) to the live connection label. */}
          <span suppressHydrationWarning data-testid="wallet-connection-state">
            {!mounted ? (
              <Badge variant="secondary">Read-only mode</Badge>
            ) : isConnected ? (
              <Badge variant="success">Connected</Badge>
            ) : (
              <Badge variant="secondary">Disconnected</Badge>
            )}
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Hard safety posture — always shown, never conditional on connection. */}
        <div className="flex flex-wrap gap-2" data-testid="wallet-safety-badges">
          <Badge variant="info">Read-only mode</Badge>
          <Badge variant="info">Paper mode</Badge>
          <Badge variant="success">Live disabled</Badge>
          <Badge variant="success">Capital exposure 0</Badge>
          <Badge variant="success">Broadcast disabled</Badge>
        </div>

        <dl className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <div>
            <dt className="text-xs uppercase tracking-widest text-muted-foreground/70">Address</dt>
            <dd className="mt-1 font-mono text-sm" suppressHydrationWarning data-testid="wallet-address">
              {mounted && isConnected && address ? abbreviate(address) : "—"}
            </dd>
          </div>
          <div>
            <dt className="text-xs uppercase tracking-widest text-muted-foreground/70">Network</dt>
            <dd className="mt-1 text-sm" suppressHydrationWarning data-testid="wallet-chain">
              {!mounted || !isConnected ? (
                "—"
              ) : !supportedChain ? (
                <Badge variant="destructive" data-testid="unsupported-chain">Unsupported chain</Badge>
              ) : (
                <span>{chainInfoSync(chainId ?? 0).name}</span>
              )}
            </dd>
          </div>
          <div>
            <dt className="text-xs uppercase tracking-widest text-muted-foreground/70">Balance (read-only)</dt>
            <dd className="mt-1 font-mono text-sm" suppressHydrationWarning data-testid="wallet-balance">
              {!mounted || !isConnected
                ? "—"
                : balance.loading
                  ? "loading…"
                  : balance.error
                    ? `error: ${balance.error}`
                    : balance.formatted
                      ? `${balance.formatted} ${balance.symbol ?? ""}`.trim()
                      : "—"}
            </dd>
          </div>
          <div>
            <dt className="text-xs uppercase tracking-widest text-muted-foreground/70">Runtime</dt>
            <dd className="mt-1 text-sm">
              {runtime ? (
                <Badge variant="outline">{runtime.mode} · read-only</Badge>
              ) : runtimeError ? (
                <Badge variant="warning">runtime unreachable</Badge>
              ) : (
                <Badge variant="secondary">…</Badge>
              )}
            </dd>
          </div>
        </dl>
      </CardContent>
    </Card>
  );
}
