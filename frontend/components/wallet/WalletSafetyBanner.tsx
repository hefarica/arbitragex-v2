"use client";

/**
 * WalletSafetyBanner — ALWAYS visible on wallet surfaces. States the hard
 * invariants the operator can rely on: read-only / paper, live disabled,
 * capital exposure 0, broadcast disabled, simulation required.
 *
 * It is purely informational and SSR-safe (no wallet-derived live values at
 * first paint). It reflects the backend safety posture when available, but the
 * displayed invariants are pinned safe regardless of any fetch outcome.
 */

import { ShieldCheckIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { useWalletSafety } from "@/hooks/useWalletSafety";

export function WalletSafetyBanner() {
  const { safety, error } = useWalletSafety();

  return (
    <div
      className="rounded-lg border border-info/40 bg-info/10 p-4"
      role="note"
      aria-label="Wallet safety posture"
      data-testid="wallet-safety-banner"
    >
      <div className="flex items-start gap-3">
        <ShieldCheckIcon className="mt-0.5 size-5 shrink-0 text-info" />
        <div className="space-y-2">
          <p className="text-sm font-medium">
            Read-only / Paper mode — this wallet surface cannot move capital.
          </p>
          <div className="flex flex-wrap gap-2">
            <Badge variant="info">Read-only mode</Badge>
            <Badge variant="info">Paper mode</Badge>
            <Badge variant="success">Live disabled</Badge>
            <Badge variant="success">Capital exposure 0</Badge>
            <Badge variant="success">Broadcast disabled</Badge>
            <Badge variant="warning">Simulation required</Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            No private keys are held server-side. No transaction is ever broadcast. Every intent
            terminates at <code className="font-mono">BROADCAST_DISABLED</code>. Connecting a wallet
            only exposes your address read-only.
          </p>
          {error && (
            <p className="text-xs text-muted-foreground/70">
              (safety endpoint unreachable: <code className="font-mono">{error}</code> — invariants
              shown above are pinned safe regardless.)
            </p>
          )}
          {!error && safety.session_mode && (
            <p className="text-xs text-muted-foreground/70">
              session mode: <code className="font-mono">{safety.session_mode}</code>
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
