/**
 * WalletConnectDegradedNotice — SERVER-RENDERED declaration of the degraded
 * WalletConnect state (PR-A, condition (c)).
 *
 * Problem it fixes: before PR-A the "WalletConnect is not configured" state was
 * only reachable *after* an operator clicked WalletConnect in the onboarding
 * guard (a client-side phase), so the SSR HTML of `/wallet` contained no trace
 * of it — the page looked fully configured while the SDK was being handed a
 * placeholder project id. R8: a degraded capability must be DECLARED, never
 * silent, never empty and never a fabricated "available".
 *
 * Server Component on purpose: the fact is a deployment fact, known on the
 * server, so it is present in the first render (no mounted-snapshot gate
 * needed) and it does not depend on any client-only SDK.
 */

import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { walletConnectEnabled, walletConnectStatus } from "@/lib/web3/walletconnect-config";

export function WalletConnectDegradedNotice() {
  const status = walletConnectStatus();
  const enabled = walletConnectEnabled();

  // Configured (or intentionally disabled by the operator) → nothing to declare.
  if (enabled && status.available) return null;

  const reason = !enabled ? "wallet_connect_disabled" : status.reason;

  return (
    <Card data-testid="walletconnect-degraded-notice">
      <CardContent className="flex flex-wrap items-center gap-3 py-4">
        <Badge variant="secondary" data-testid="walletconnect-degraded-badge">
          WalletConnect no disponible
        </Badge>
        <span className="text-sm text-muted-foreground">
          {enabled
            ? "WalletConnect / Reown no está configurado en este despliegue: no se inicializa su SDK y no se llama a ningún endpoint suyo. Puedes conectar una wallet del navegador (MetaMask / EIP-6963); el resto de la superficie de wallet sigue operativa."
            : "WalletConnect está deshabilitado por configuración en este despliegue. Puedes conectar una wallet del navegador (MetaMask / EIP-6963)."}{" "}
          <span className="font-mono" data-testid="walletconnect-degraded-reason">
            reason={reason}
          </span>
        </span>
      </CardContent>
    </Card>
  );
}
