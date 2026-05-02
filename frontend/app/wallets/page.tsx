import React from "react";

export const metadata = { title: "Wallets & Allowances | ArbitrageX" };

export default function WalletsPage() {
  return (
    <div className="p-8 space-y-6">
      <h1 className="text-3xl font-bold tracking-tight text-slate-100">Operational Wallets</h1>
      <div className="bg-slate-900 border border-slate-800 rounded-xl p-6 shadow-2xl">
        <p className="text-slate-400">Manage encrypted wallets and smart contract allowances securely.</p>
      </div>
    </div>
  );
}
