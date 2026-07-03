"use client";

/**
 * WalletConnectConfirm — shown when the selected wallet IS installed/detected. Asks the operator to
 * explicitly confirm connecting. Pure presentational (props only) → SSR-static testable. Connection
 * happens ONLY on an explicit click (the parent wires onConnect to wagmi). The permissions list is the
 * hard contract: read address + read chain only; NO signing, NO approvals, NO broadcast. The DApp never
 * auto-connects and never auto-clicks inside the wallet.
 */

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type { WalletInstallRegistryEntry } from "@/lib/web3/wallet-registry";

export function WalletConnectConfirm({
  entry,
  connecting,
  onConnect,
  onCancel,
}: {
  entry: WalletInstallRegistryEntry;
  connecting: boolean;
  onConnect: () => void;
  onCancel: () => void;
}) {
  return (
    <Card data-testid="wallet-connect-confirm">
      <CardHeader>
        <CardTitle>Wallet detectada</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <p className="text-sm">
          <strong>{entry.name}</strong> está instalada y disponible. ¿Deseas conectarla a la DApp?
        </p>

        <div>
          <div className="mb-2 text-xs uppercase tracking-widest text-muted-foreground/70">Permisos</div>
          <ul className="space-y-1 text-sm" data-testid="wallet-permissions">
            <li>Leer la dirección pública.</li>
            <li>Leer la red / chain.</li>
            <li>Solicitar cambio de red solo si lo confirmas.</li>
            <li className="text-muted-foreground/80">NO firma transacciones.</li>
            <li className="text-muted-foreground/80">NO aprueba tokens.</li>
            <li className="text-muted-foreground/80">NO hace broadcast.</li>
          </ul>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="default"
            onClick={onConnect}
            disabled={connecting}
            aria-disabled={connecting}
            data-testid="wallet-connect"
          >
            {connecting ? "Conectando…" : "Conectar wallet"}
          </Button>
          <Button type="button" variant="ghost" onClick={onCancel} data-testid="wallet-connect-cancel">
            Cancelar
          </Button>
        </div>

        <Badge variant="info" data-testid="wallet-connect-mode">
          La wallet queda en modo read-only / policy-gated. No se firma ni se mueve capital.
        </Badge>
      </CardContent>
    </Card>
  );
}
