"use client";

/**
 * REJECT-BREAKDOWN-EXPORT-01 — grouped rejection-reason panel.
 *
 * The operator's one-by-one remediation surface: every rejection_reason
 * family with its share of the window, avg gross/net, the TokenNotAllowed
 * address flood with resolved symbols, and a CSV download of the whole
 * breakdown (both sections in one file) for offline pivoting.
 *
 * Honesty contract (RULE 00 / R8): avg_gross/avg_net null renders "—"
 * (not computed for that family), never 0. Unknown token address renders
 * its raw address (symbol null is honest). Errors render verbatim.
 *
 * R1: client-only data fetching (useEffect, no SSR snapshot); the CSV is
 * built in-page from ALREADY-fetched data — no second request, no mock.
 */
// Classic-JSX runtime import for the vitest (esbuild) path — repo pattern
// (PairIntelligencePanel.tsx, ByModeKpiStrip.tsx); Next/swc ignores it.
import * as React from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fetchRejectionBreakdown, type RejectionBreakdown } from "@/lib/api-client";

const WINDOWS = [
  { label: "24 h", hours: 24 },
  { label: "7 d", hours: 168 },
  { label: "30 d", hours: 720 },
] as const;

function fmtUsd(v: number | null): string {
  if (v === null || !Number.isFinite(v)) return "—";
  return `$${v.toFixed(v >= 100 ? 0 : 2)}`;
}

function fmtInt(v: number): string {
  return v.toLocaleString("en-US");
}

function shortAddr(a: string): string {
  return `${a.slice(0, 8)}…${a.slice(-6)}`;
}

function csvEscape(v: string): string {
  if (/[",\r\n]/.test(v)) return `"${v.replaceAll('"', '""')}"`;
  return v;
}

/** CWE-1236 (CSV formula injection): token symbols/families are on-chain or
 * writer-derived strings — a malicious token COULD set symbol
 * "=HYPERLINK(...)". Excel/Sheets execute cells starting with = + - @ or a
 * tab; neutralize with a leading apostrophe (OWASP guidance). */
function csvSafe(v: string): string {
  return /^[=+\-@\t\r]/.test(v) ? `'${v}` : v;
}

/** One CSV with both sections: families + token flood. Data-only download of
 * what the panel already shows — nothing fabricated, nulls stay empty. BOM
 * first so Excel decodes UTF-8 (on-chain symbols carry non-ASCII).
 * Exported for unit tests (pure function). */
export function buildCsv(d: RejectionBreakdown): string {
  const lines: string[] = [];
  lines.push(`# rejection_breakdown window_hours=${d.window_hours} chain_id=${d.chain_id ?? "all"} generated_at=${d.generated_at}`);
  lines.push(`# total_rows=${d.total_rows} rejected_rows=${d.rejected_rows}`);
  if (d.raw_groups_truncated) {
    lines.push("# raw_groups_truncated=true — family counts cover the top 500 raw reasons only; rejected_rows is exact");
  }
  lines.push("section,family_or_token,count,share_pct_of_rejected,avg_gross_usd,avg_net_usd");
  for (const f of d.families) {
    lines.push(
      [
        "family",
        csvSafe(f.family),
        String(f.count),
        String(f.share_pct_of_rejected),
        f.avg_gross_usd === null ? "" : String(f.avg_gross_usd),
        f.avg_net_usd === null ? "" : String(f.avg_net_usd),
      ]
        .map(csvEscape)
        .join(","),
    );
  }
  for (const t of d.token_flood) {
    lines.push(
      ["token_flood", csvSafe(t.symbol ?? t.address), String(t.count), "", "", ""].map(csvEscape).join(","),
    );
  }
  return `\uFEFF${lines.join("\r\n")}\r\n`;
}

/**
 * Pure presentational view (repo test pattern: node env, renderToStaticMarkup
 * — see ByModeKpiStrip.test.tsx). The container below owns the fetch.
 */
export function RejectionBreakdownView(props: {
  data: RejectionBreakdown | null;
  error: string | null;
  hours: number;
  onHoursChange: (h: number) => void;
  onDownloadCsv: () => void;
}) {
  const { data, error, hours, onHoursChange, onDownloadCsv } = props;
  const rejectedPct =
    data && data.total_rows > 0 ? Math.round((data.rejected_rows / data.total_rows) * 1000) / 10 : null;

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="text-sm font-medium text-muted-foreground">
            Rechazos por razón · desglose agrupado
          </CardTitle>
          <div className="flex items-center gap-2">
            {rejectedPct !== null && (
              <Badge variant={rejectedPct >= 99 ? "destructive" : "secondary"}>
                {rejectedPct}% rechazadas
              </Badge>
            )}
            <div className="flex overflow-hidden rounded-md border" role="group" aria-label="Ventana del desglose">
              {WINDOWS.map((w) => (
                <Button
                  key={w.hours}
                  size="sm"
                  variant={hours === w.hours ? "default" : "ghost"}
                  className="h-7 rounded-none px-2 text-xs first:rounded-l-md last:rounded-r-md"
                  onClick={() => onHoursChange(w.hours)}
                  aria-pressed={hours === w.hours}
                >
                  {w.label}
                </Button>
              ))}
            </div>
            <Button size="sm" variant="outline" disabled={!data} onClick={() => onDownloadCsv()}>
              Descargar CSV
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {error && (
          <p className="text-xs text-destructive" role="alert">
            {error}
          </p>
        )}
        {!data && !error && <p className="text-xs text-muted-foreground">Cargando…</p>}

        {data && (
          <>
            <p className="text-xs text-muted-foreground">
              {fmtInt(data.rejected_rows)} rechazadas de {fmtInt(data.total_rows)} oportunidades ·{" "}
              {data.window_hours} h
            </p>
            {data.raw_groups_truncated && (
              <p className="text-xs text-amber-600 dark:text-amber-400">
                ⚠ Más de 500 razones crudas en la ventana — los conteos por familia cubren sólo las
                500 mayores; rechazadas totales es exacto. Reduce la ventana para el detalle completo.
              </p>
            )}
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <th className="py-1.5 pr-3 font-medium">Familia</th>
                    <th className="py-1.5 pr-3 font-medium">Rechazos</th>
                    <th className="py-1.5 pr-3 font-medium">% del total</th>
                    <th className="py-1.5 pr-3 font-medium">Gross prom.</th>
                    <th className="py-1.5 pr-3 font-medium">Net prom.</th>
                  </tr>
                </thead>
                <tbody>
                  {data.families.map((f) => (
                    <tr key={f.family} className="border-b last:border-0">
                      <td className="py-1.5 pr-3 font-mono">{f.family}</td>
                      <td className="py-1.5 pr-3 font-mono tabular-nums">{fmtInt(f.count)}</td>
                      <td className="py-1.5 pr-3 font-mono tabular-nums">{f.share_pct_of_rejected}%</td>
                      <td className="py-1.5 pr-3 font-mono tabular-nums">{fmtUsd(f.avg_gross_usd)}</td>
                      <td className="py-1.5 pr-3 font-mono tabular-nums">{fmtUsd(f.avg_net_usd)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {data.token_flood.length > 0 && (
              <div className="overflow-x-auto">
                <div className="mb-1 text-xs font-medium text-muted-foreground">
                  Inundación por token (TokenNotAllowed)
                </div>
                <table className="w-full text-xs">
                  <thead>
                    <tr className="border-b text-left text-muted-foreground">
                      <th className="py-1.5 pr-3 font-medium">Token</th>
                      <th className="py-1.5 pr-3 font-medium">Dirección</th>
                      <th className="py-1.5 pr-3 font-medium">Rechazos</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.token_flood.map((t) => (
                      <tr key={t.address} className="border-b last:border-0">
                        <td className="py-1.5 pr-3 font-mono">{t.symbol ?? "—"}</td>
                        <td className="py-1.5 pr-3 font-mono" title={t.address}>
                          {shortAddr(t.address)}
                        </td>
                        <td className="py-1.5 pr-3 font-mono tabular-nums">{fmtInt(t.count)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </>
        )}
      </CardContent>
    </Card>
  );
}

/** Fetch container (R1: client-only data fetching, no SSR snapshot). */
export function RejectionBreakdownPanel() {
  const [hours, setHours] = useState<number>(24);
  const [data, setData] = useState<RejectionBreakdown | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Latest-wins: a slow response for an abandoned window must never overwrite
  // the data of the window the operator switched to.
  const reqSeq = useRef(0);

  const refresh = useCallback(async (h: number) => {
    const seq = ++reqSeq.current;
    const r = await fetchRejectionBreakdown(h);
    if (seq !== reqSeq.current) return; // superseded by a newer window request
    if (r.ok) {
      setData(r.data);
      setError(null);
    } else {
      setError(r.error); // verbatim — keep last good data visible
    }
  }, []);

  useEffect(() => {
    void refresh(hours);
  }, [refresh, hours]);

  const onDownloadCsv = () => {
    if (!data) return;
    const blob = new Blob([buildCsv(data)], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `rejection_breakdown_${data.window_hours}h_${data.generated_at.slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <RejectionBreakdownView
      data={data}
      error={error}
      hours={hours}
      onHoursChange={setHours}
      onDownloadCsv={onDownloadCsv}
    />
  );
}
