/**
 * Runtime Cartridges summary — compact card for /config/trading showing how
 * many .rhai strategy cartridges are actually loaded on the searcher hot-path
 * (the 264-strategy library + core pack), with a link to the full management
 * view in /strategies → Runtime Cartridges tab.
 *
 * Data: GET /api/cartridges/runtime (searcher registry snapshot). R8
 * fail-honest: unavailable registry shows an honest "unavailable", never a
 * fabricated count. Client component (polls every 4s).
 */
"use client";

import { useEffect, useState } from "react";
import Link from "next/link";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { getRuntimeCartridges } from "@/lib/api-client";

const POLL_MS = 4000;

export function RuntimeCartridgesSummary({ chainId }: { chainId: number }) {
  const [total, setTotal] = useState<number | null>(null);
  const [active, setActive] = useState<number | null>(null);
  const [available, setAvailable] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      const r = await getRuntimeCartridges(chainId);
      if (cancelled) return;
      if (r.ok && r.data.ok && r.data.data) {
        setTotal(r.data.data.total);
        setActive(r.data.data.active);
        setAvailable(true);
      } else {
        setTotal(null);
        setActive(null);
        setAvailable(false);
      }
    };
    void load();
    const id = setInterval(() => void load(), POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [chainId]);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-3">
          Runtime cartridges
          {available && total != null && (
            <Badge variant="outline" className="bg-success/15 text-success border-success/40 text-[10px] font-bold">
              {active} active / {total} loaded
            </Badge>
          )}
          {available === false && (
            <Badge variant="outline" className="bg-muted text-muted-foreground border-border text-[10px] font-bold">
              unavailable
            </Badge>
          )}
        </CardTitle>
        <CardDescription>
          Live .rhai strategy cartridges the searcher compiled on its hot-path
          (the 264-strategy library + core pack). Toggle individual cartridges
          on/off via Redis hot-reload — no restart.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {available && total != null ? (
          <p className="text-sm text-muted-foreground">
            <span className="font-mono text-success font-semibold">{active}</span> active ·{" "}
            <span className="font-mono text-foreground font-semibold">{total}</span> loaded on chain{" "}
            {chainId}. Manage them in{" "}
            <Link href="/strategies" className="underline">
              /strategies → Runtime Cartridges
            </Link>
            .
          </p>
        ) : (
          <p className="text-sm text-muted-foreground">
            {available === false
              ? "Searcher registry unavailable (down, boot pending, or TTL expired). R8 fail-honest: no count fabricated."
              : "Loading runtime cartridge registry…"}
          </p>
        )}
      </CardContent>
    </Card>
  );
}
