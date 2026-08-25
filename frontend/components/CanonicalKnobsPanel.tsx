"use client";

/**
 * Ω FE-MASTER · CanonicalKnobsPanel (FE-0061 / FE-CFG-001..003)
 *
 * Excel 01_CONFIG (17 parameters) mapped against the searcher-rs canonical
 * knob snapshot. READ-ONLY surface: classification is EFFECTIVE / DERIVED /
 * NOT EXPOSED — a missing runtime binding renders "—", NEVER a fabricated
 * zero (R8 / FE-CFG-003). Mutation contracts (FE-CFG-005..007) do not exist
 * by design; values change via env/deploy-yaml + searcher boot.
 *
 * Port-with-validation of the overlay reference onto repo conventions:
 * extended the binding map 5 → 12 keys validated against
 * searcher-rs/src/canonical_knobs.rs; wire schema is
 * CanonicalKnobsResponseSchema (apex), not the overlay's loose variant.
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { getCanonicalKnobs } from "@/lib/api-client";
import { buildKnobRows, type KnobRowStatus } from "@/lib/apex/config-spec";
import type { CanonicalKnobsResponse } from "@/lib/apex/schemas";

// Repo badge variants only (validated against components/ui/badge.tsx).
const STATUS_VARIANT: Record<KnobRowStatus, "secondary" | "info" | "outline"> = {
  EFFECTIVE: "secondary",
  DERIVED: "info",
  NOT_EXPOSED: "outline",
};

export function CanonicalKnobsPanel() {
  const [snapshot, setSnapshot] = useState<CanonicalKnobsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    const r = await getCanonicalKnobs();
    setLoading(false);
    if (r.ok) {
      setSnapshot(r.data);
      setError(null);
    } else {
      // 503 knobs_not_published / redis_unavailable — honest absence.
      setSnapshot(null);
      setError(r.error);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const rows = useMemo(() => buildKnobRows(snapshot), [snapshot]);
  const effectiveCount = useMemo(
    () => rows.filter((r) => r.status === "EFFECTIVE").length,
    [rows],
  );

  return (
    <Card className="mt-6 py-0">
      <CardHeader>
        <div className="flex items-start justify-between gap-4">
          <div>
            <CardTitle>Canonical runtime bindings · workbook 01_CONFIG</CardTitle>
            <CardDescription>
              Excel parameters vs the searcher-rs boot snapshot (env ARBX_KNOB_* &gt; deploy
              yaml &gt; workbook). Missing bindings are shown explicitly — never fabricated,
              never zero.
            </CardDescription>
          </div>
          <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
            <RefreshCw className={`mr-2 h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
            Refresh
          </Button>
        </div>
        <div className="flex flex-wrap gap-2 text-xs">
          <Badge variant="outline">{rows.length} Excel parameters</Badge>
          <Badge variant="outline">{effectiveCount}/{rows.length} EFFECTIVE</Badge>
          <Badge variant="outline">source: {snapshot?.source ?? "unavailable"}</Badge>
          {snapshot && (
            <Badge variant="outline">boot snapshot: {snapshot.generated_at}</Badge>
          )}
          {error && <Badge variant="destructive">runtime snapshot unavailable</Badge>}
        </div>
      </CardHeader>
      <CardContent className="p-0">
        {error && (
          <p className="px-6 pb-4 font-mono text-xs text-destructive" role="alert">
            {error}
          </p>
        )}
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Parameter</TableHead>
                <TableHead>Excel value</TableHead>
                <TableHead>Effective runtime</TableHead>
                <TableHead>Unit</TableHead>
                <TableHead>Runtime binding</TableHead>
                <TableHead>Status</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map(({ spec, knobKey, effective, status }) => {
                const effectiveExists = effective !== undefined;
                return (
                  <TableRow key={spec.Parameter}>
                    <TableCell className="font-mono text-xs">{spec.Parameter}</TableCell>
                    <TableCell className="font-mono text-xs tabular-nums">
                      {String(spec.Value ?? "—")}
                    </TableCell>
                    <TableCell className="font-mono text-xs tabular-nums">
                      {effectiveExists ? String(effective) : "—"}
                    </TableCell>
                    <TableCell className="text-xs text-muted-foreground">
                      {spec.Unit || "—"}
                    </TableCell>
                    <TableCell className="font-mono text-xs text-muted-foreground">
                      {knobKey ?? spec["Runtime binding"] ?? "—"}
                    </TableCell>
                    <TableCell>
                      <Badge variant={STATUS_VARIANT[status]} className="text-[10px]">
                        {status}
                      </Badge>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>
  );
}
