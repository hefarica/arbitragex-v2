"use client";

import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "@/components/ui/sheet";
import { Badge } from "@/components/ui/badge";
import type { DexRow } from "@/lib/api/dexes";

interface Props {
  dex: DexRow | null;
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

function fmtUsd(v: number | null): string {
  if (v == null) return "—";
  if (v >= 1_000_000_000) return `$${(v / 1_000_000_000).toFixed(2)}B`;
  if (v >= 1_000_000) return `$${(v / 1_000_000).toFixed(2)}M`;
  if (v >= 1_000) return `$${(v / 1_000).toFixed(2)}K`;
  return `$${v.toFixed(2)}`;
}

export function DexDetailDialog({ dex, onClose }: Props) {
  return (
    <Sheet open={dex !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-[400px] sm:w-[540px] overflow-y-auto">
        <SheetHeader className="mb-6">
          <SheetTitle>DEX Detail</SheetTitle>
          <SheetDescription>
            {dex
              ? `${dex.name} · ${dex.protocol_type}`
              : ""}
          </SheetDescription>
        </SheetHeader>

        {dex && (
          <div className="space-y-1">
            <Row label="ID" value={dex.id} />
            <Row label="Name" value={dex.name} />
            <Row label="Protocol" value={dex.protocol_type} />
            <Row
              label="Status"
              value={
                dex.is_active ? (
                  <Badge variant="success">Active</Badge>
                ) : (
                  <Badge variant="secondary">Disabled</Badge>
                )
              }
            />
            <Row
              label="Chains"
              value={
                dex.chain_ids.length > 0 ? (
                  <div className="flex flex-wrap gap-1 mt-0.5">
                    {dex.chain_ids.map((c) => (
                      <Badge key={c} variant="outline">
                        {c}
                      </Badge>
                    ))}
                  </div>
                ) : (
                  "—"
                )
              }
            />
            <Row label="Volume 24h" value={fmtUsd(dex.volume_24h_usd)} />
            <Row label="TVL" value={fmtUsd(dex.tvl_usd)} />
            <Row
              label="Fee"
              value={dex.fee_bps != null ? `${dex.fee_bps} bps` : null}
            />
            <Row label="Router" value={dex.router_address} />
            <Row label="Factory" value={dex.factory_address} />
            <Row label="Created" value={dex.created_at} />
          </div>
        )}
      </SheetContent>
    </Sheet>
  );
}
