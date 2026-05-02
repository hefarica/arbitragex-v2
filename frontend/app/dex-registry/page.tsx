import React from "react";

export const metadata = { title: "DEX Registry | ArbitrageX" };

export default function DexRegistryPage() {
  return (
    <div className="p-8 space-y-6">
      <h1 className="text-3xl font-bold tracking-tight text-slate-100">DEX & Token Allowlist</h1>
      <div className="bg-slate-900 border border-slate-800 rounded-xl p-6 shadow-2xl">
        <p className="text-slate-400">Loading authorized DEXes (UniV2, UniV3, Curve)...</p>
      </div>
    </div>
  );
}
