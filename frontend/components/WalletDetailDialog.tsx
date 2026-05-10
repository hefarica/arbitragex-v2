"use client";

import { useState, useEffect } from "react";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "@/components/ui/sheet";
import { Badge } from "@/components/ui/badge";
import { AlertCircle, Loader2 } from "lucide-react";
import type { WalletRow, WalletBalance, WalletAllowance } from "@/lib/api/wallets";
import { getWalletBalances, getWalletAllowances } from "@/lib/api/wallets";

interface Props {
  wallet: WalletRow | null;
  onClose: () => void;
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5 py-2 border-b border-border last:border-0">
      <span className="text-xs text-muted-foreground uppercase tracking-wide">
        {label}
      </span>
      <span className="text-sm font-mono break-all">
        {value ?? (
          <span className="text-muted-foreground/50 italic">—</span>
        )}
      </span>
    </div>
  );
}

function EndpointNotice({ message }: { message: string }) {
  return (
    <div className="flex items-start gap-2 rounded-md border border-warning/40 bg-warning/10 p-3 text-xs text-warning">
      <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
      <span>{message}</span>
    </div>
  );
}

export function WalletDetailDialog({ wallet, onClose }: Props) {
  const EDGE_URL =
    process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";

  const [balances, setBalances] = useState<WalletBalance[] | null>(null);
  const [allowances, setAllowances] = useState<WalletAllowance[] | null>(null);
  const [balError, setBalError] = useState<string | null>(null);
  const [allowError, setAllowError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // R1: all side-effects in useEffect — never in render
  useEffect(() => {
    if (!wallet) {
      setBalances(null);
      setAllowances(null);
      setBalError(null);
      setAllowError(null);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setBalances(null);
    setAllowances(null);
    setBalError(null);
    setAllowError(null);

    Promise.all([
      getWalletBalances(EDGE_URL, wallet.address),
      getWalletAllowances(EDGE_URL, wallet.address),
    ]).then(([balRes, allowRes]) => {
      if (cancelled) return;
      if (balRes.ok) {
        setBalances(balRes.data.balances);
      } else {
        setBalError(balRes.error);
      }
      if (allowRes.ok) {
        setAllowances(allowRes.data.allowances);
      } else {
        setAllowError(allowRes.error);
      }
      setLoading(false);
    });

    return () => {
      cancelled = true;
    };
  }, [wallet, EDGE_URL]);

  return (
    <Sheet open={wallet !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-[400px] sm:w-[580px] overflow-y-auto">
        <SheetHeader className="mb-6">
          <SheetTitle>Wallet Detail</SheetTitle>
          <SheetDescription>
            {wallet ? `${wallet.label} · ${wallet.role ?? "no role"}` : ""}
          </SheetDescription>
        </SheetHeader>

        {wallet && (
          <div className="space-y-6">
            {/* Identity */}
            <section>
              <h3 className="text-xs font-semibold uppercase tracking-widest text-muted-foreground mb-2">
                Identity
              </h3>
              <div className="space-y-0">
                <Row label="Address" value={wallet.address} />
                <Row label="Label" value={wallet.label} />
                <Row
                  label="Role"
                  value={
                    wallet.role ? (
                      <Badge variant="outline">{wallet.role}</Badge>
                    ) : null
                  }
                />
              </div>
            </section>

            {/* Balances */}
            <section>
              <h3 className="text-xs font-semibold uppercase tracking-widest text-muted-foreground mb-2">
                Balances
              </h3>
              {loading && (
                <div className="flex items-center gap-2 text-muted-foreground text-xs py-2">
                  <Loader2 className="size-3 animate-spin" />
                  Fetching balances…
                </div>
              )}
              {balError && <EndpointNotice message={balError} />}
              {balances && balances.length === 0 && (
                <p className="text-xs text-muted-foreground italic">
                  No balances returned.
                </p>
              )}
              {balances && balances.length > 0 && (
                <div className="rounded-md border border-border overflow-hidden">
                  <table className="w-full text-xs">
                    <thead>
                      <tr className="bg-muted text-muted-foreground uppercase text-[10px] tracking-wider">
                        <th className="px-3 py-2 text-left">Token</th>
                        <th className="px-3 py-2 text-left">Chain</th>
                        <th className="px-3 py-2 text-right">Balance</th>
                        <th className="px-3 py-2 text-right">USD</th>
                      </tr>
                    </thead>
                    <tbody>
                      {balances.map((b, i) => (
                        <tr
                          key={`${b.chain_id}-${b.token_address ?? b.token_symbol}-${i}`}
                          className="border-t border-border"
                        >
                          <td className="px-3 py-2 font-mono font-semibold">
                            {b.token_symbol}
                          </td>
                          <td className="px-3 py-2 text-muted-foreground">
                            {b.chain_name ?? b.chain_id}
                          </td>
                          <td className="px-3 py-2 text-right font-mono">
                            {b.balance_formatted ?? b.balance_raw}
                          </td>
                          <td className="px-3 py-2 text-right text-muted-foreground">
                            {b.balance_usd != null
                              ? `$${b.balance_usd.toFixed(2)}`
                              : "—"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </section>

            {/* Allowances */}
            <section>
              <h3 className="text-xs font-semibold uppercase tracking-widest text-muted-foreground mb-2">
                Allowances
              </h3>
              {loading && !balError && (
                <div className="flex items-center gap-2 text-muted-foreground text-xs py-2">
                  <Loader2 className="size-3 animate-spin" />
                  Fetching allowances…
                </div>
              )}
              {allowError && <EndpointNotice message={allowError} />}
              {allowances && allowances.length === 0 && (
                <p className="text-xs text-muted-foreground italic">
                  No allowances returned.
                </p>
              )}
              {allowances && allowances.length > 0 && (
                <div className="rounded-md border border-border overflow-hidden">
                  <table className="w-full text-xs">
                    <thead>
                      <tr className="bg-muted text-muted-foreground uppercase text-[10px] tracking-wider">
                        <th className="px-3 py-2 text-left">Token</th>
                        <th className="px-3 py-2 text-left">Spender</th>
                        <th className="px-3 py-2 text-right">Amount</th>
                      </tr>
                    </thead>
                    <tbody>
                      {allowances.map((a, i) => (
                        <tr
                          key={`${a.token_address}-${a.spender_address}-${i}`}
                          className="border-t border-border"
                        >
                          <td className="px-3 py-2 font-mono font-semibold">
                            {a.token_symbol ?? a.token_address.slice(0, 8) + "…"}
                          </td>
                          <td className="px-3 py-2 text-muted-foreground font-mono">
                            {a.spender_label ??
                              a.spender_address.slice(0, 10) + "…"}
                          </td>
                          <td className="px-3 py-2 text-right">
                            {a.is_unlimited ? (
                              <Badge variant="destructive">UNLIMITED</Badge>
                            ) : (
                              <span className="font-mono">
                                {a.allowance_formatted ?? a.allowance_raw}
                              </span>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </section>
          </div>
        )}
      </SheetContent>
    </Sheet>
  );
}
