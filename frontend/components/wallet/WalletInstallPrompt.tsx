"use client";

/**
 * WalletInstallPrompt — shown when the operator selects a wallet that is NOT installed/detected.
 *
 * Pure presentational (props only, no hooks) so it is SSR-static testable. The "official download" link
 * is a plain anchor whose href comes ONLY from the static registry (installUrlFor) and always opens in a
 * new tab with rel="noopener noreferrer". The DApp installs NOTHING, downloads NOTHING, and asks for NO
 * seed phrase / private key — it only points the operator to the official site.
 */

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { installUrlFor, type WalletInstallRegistryEntry } from "@/lib/web3/wallet-registry";

export function WalletInstallPrompt({
  entry,
  onRedetect,
  onCancel,
}: {
  entry: WalletInstallRegistryEntry;
  onRedetect: () => void;
  onCancel: () => void;
}) {
  // Allowlisted URL ONLY — never a dynamic or caller-supplied URL.
  const url = installUrlFor(entry.id);

  return (
    <Card data-testid="wallet-install-prompt">
      <CardHeader>
        <CardTitle>Wallet no instalada o no detectada</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <p className="text-sm">
          La wallet seleccionada (<strong>{entry.name}</strong>) no se encuentra instalada, habilitada o
          detectada en este equipo/navegador.
        </p>

        <div className="flex flex-wrap gap-2">
          {url ? (
            <a href={url} target="_blank" rel="noopener noreferrer" data-testid="wallet-install-link">
              <Button type="button" variant="default">
                Ir al sitio oficial de descarga
              </Button>
            </a>
          ) : null}
          <Button type="button" variant="outline" onClick={onRedetect} data-testid="wallet-redetect">
            Volver a detectar
          </Button>
          <Button type="button" variant="ghost" onClick={onCancel} data-testid="wallet-cancel">
            Cancelar
          </Button>
        </div>

        <Badge variant="warning" data-testid="wallet-seed-warning">
          Nunca ingreses tu seed phrase o private key en esta DApp. Instala la wallet únicamente desde el
          sitio oficial ({entry.warning})
        </Badge>
      </CardContent>
    </Card>
  );
}
