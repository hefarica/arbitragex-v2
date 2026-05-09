import React from "react";

export const metadata = { title: "Wallets & Allowances | ArbitrageX" };

export default function WalletsPage() {
  return (
    <div className="p-8 space-y-6">
      <h1 className="text-3xl font-bold tracking-tight text-foreground">Operational Wallets</h1>
      <div data-slot="card" className="bg-card text-card-foreground border border-border rounded-xl p-6 shadow-2xl">
        <p className="text-muted-foreground">Manage encrypted wallets and smart contract allowances securely.</p>
      </div>
    </div>
  );
}
