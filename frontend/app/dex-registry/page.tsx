import React from "react";

export const metadata = { title: "DEX Registry | ArbitrageX" };

export default function DexRegistryPage() {
  return (
    <div className="p-8 space-y-6">
      <h1 className="text-3xl font-bold tracking-tight text-foreground">DEX & Token Allowlist</h1>
      <div data-slot="card" className="bg-card text-card-foreground border border-border rounded-xl p-6 shadow-2xl">
        <p className="text-muted-foreground">Loading authorized DEXes (UniV2, UniV3, Curve)...</p>
      </div>
    </div>
  );
}
