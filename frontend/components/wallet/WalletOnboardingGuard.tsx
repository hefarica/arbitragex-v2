"use client";

/**
 * WalletOnboardingGuard — orchestrates the safe onboarding flow:
 *   select wallet → (not detected) WalletInstallPrompt → official site → re-detect
 *                → (detected)     WalletConnectConfirm → explicit connect (read-only/policy-gated)
 *
 * Connection happens ONLY on an explicit operator click (never auto-connect, never auto-click inside the
 * wallet). No signer, no key, no signTypedData, no approvals, no broadcast. Chain switching stays with
 * the existing NetworkSwitcher on the wallet page. SSR-safe via a mounted gate.
 */

import * as React from "react";
import { useAccount, useConnect } from "wagmi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { WALLET_INSTALL_REGISTRY } from "@/lib/web3/wallet-registry";
import { useWalletDiscovery } from "@/hooks/useWalletDiscovery";
import { useWalletOnboarding } from "@/hooks/useWalletOnboarding";
import { WalletInstallPrompt } from "./WalletInstallPrompt";
import { WalletConnectConfirm } from "./WalletConnectConfirm";

export function WalletOnboardingGuard() {
  const [mounted, setMounted] = React.useState(false);
  React.useEffect(() => setMounted(true), []);

  const { providers, redetect } = useWalletDiscovery();
  const { phase, entry, select, cancel } = useWalletOnboarding(providers);
  const { connect, connectors, isPending } = useConnect();
  const { isConnected } = useAccount();

  // Explicit user-click connect ONLY. Match a wagmi connector by id (best-effort); never auto-connect.
  const onConnect = (): void => {
    if (!entry) return;
    const c = connectors.find(
      (x) => x.id === entry.connectorId || x.name.toLowerCase().includes(entry.name.toLowerCase()),
    );
    if (c) connect({ connector: c });
  };

  return (
    <Card data-testid="wallet-onboarding-guard">
      <CardHeader>
        <CardTitle>Conectar una wallet</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {!mounted ? (
          <Badge variant="secondary" data-testid="wallet-onboarding-shell">
            …
          </Badge>
        ) : isConnected ? (
          <Badge variant="success" data-testid="wallet-onboarding-connected">
            Wallet conectada — modo read-only / policy-gated.
          </Badge>
        ) : phase === "idle" ? (
          <div className="flex flex-wrap gap-2" data-testid="wallet-selector">
            {WALLET_INSTALL_REGISTRY.map((w) => (
              <Button
                key={w.id}
                type="button"
                variant="outline"
                onClick={() => select(w.id)}
                data-testid={`select-${w.id}`}
              >
                {w.name}
              </Button>
            ))}
          </div>
        ) : phase === "install" && entry ? (
          <WalletInstallPrompt entry={entry} onRedetect={redetect} onCancel={cancel} />
        ) : phase === "connect" && entry ? (
          <WalletConnectConfirm entry={entry} connecting={isPending} onConnect={onConnect} onCancel={cancel} />
        ) : null}
      </CardContent>
    </Card>
  );
}
