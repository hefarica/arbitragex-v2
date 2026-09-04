"use client";

/**
 * DAPP-ARCHIVE-UI-01 — Cold-tier archive panel (ARBX-RETENTION-01).
 *
 * Operator surface for the retention policy's archive leg
 * (docs/RETENTION_POLICY.md): live disk capacity of the archives mount,
 * per-table rows beyond the retention window, a MANUAL export trigger, and
 * the AUTOMATIC mode toggle (nightly cron archives each range before
 * purging — fail-honest: archive failed → no purge that night).
 *
 * Honesty contract (RULE 00 / R8): rows_beyond_window === null renders "—"
 * (not computed), never 0. Every error renders verbatim. Empty file list =
 * empty list. No fabricated state, no optimistic toggles — the switch
 * reflects the server's persisted value on the next status fetch.
 *
 * R1: client-only data fetching (useEffect + 30s poll); no SSR snapshot, no
 * hydration-sensitive values rendered before mount.
 */
import { useCallback, useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  fetchArchiveStatus,
  postArchiveAuto,
  postArchiveExport,
  type ArchiveStatus,
} from "@/lib/api-client";

const POLL_MS = 30_000;

function fmtBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes)) return "—";
  const gb = bytes / 1024 ** 3;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  return `${(bytes / 1024 ** 2).toFixed(0)} MB`;
}

function fmtWhen(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  return d.toISOString().slice(0, 16).replace("T", " ") + "Z";
}

export function ArchivePanel() {
  const [status, setStatus] = useState<ArchiveStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionMsg, setActionMsg] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const r = await fetchArchiveStatus();
    if (r.ok) {
      setStatus(r.data);
      setError(null);
    } else {
      // keep last good status; surface the failure verbatim
      setError(r.error);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    const tick = async () => {
      if (cancelled) return;
      void refresh();
    };
    void tick();
    const id = setInterval(tick, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [refresh]);

  const onToggleAuto = async (next: boolean) => {
    setBusy(true);
    setActionMsg(null);
    const r = await postArchiveAuto(next);
    if (r.ok) {
      setActionMsg(`Modo automático ${next ? "activado" : "desactivado"}`);
    } else {
      setActionMsg(`Error: ${r.error}`);
    }
    await refresh();
    setBusy(false);
  };

  const onExport = async (table: string) => {
    setBusy(true);
    setActionMsg(null);
    const r = await postArchiveExport(table);
    if (r.ok) {
      setActionMsg(`Exportación de ${table} iniciada (observa la capacidad y los archivos)`);
    } else {
      setActionMsg(`Error: ${r.error}`);
    }
    // 202-accepted: the export runs detached — refresh now + shortly after so
    // the file (or its absence) surfaces honestly.
    await refresh();
    setTimeout(() => void refresh(), 8000);
    setBusy(false);
  };

  const disk = status?.disk;
  const usedPct = disk && "used_pct" in disk ? disk.used_pct : null;
  const freeBytes = disk && "free_bytes" in disk ? disk.free_bytes : null;
  const totalBytes = disk && "total_bytes" in disk ? disk.total_bytes : null;
  const exportRunning = status?.export_running ?? null;

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="text-sm font-medium text-muted-foreground">
            Archivo Frío · retención de datos
          </CardTitle>
          <div className="flex items-center gap-2">
            <Badge variant={status?.auto_mode?.enabled ? "default" : "secondary"}>
              {status?.auto_mode?.enabled ? "auto: ON" : "auto: OFF"}
            </Badge>
            {exportRunning && (
              <Badge variant="outline">exportando {exportRunning.table}…</Badge>
            )}
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Capacity — the operator watches disk before/while exporting */}
        <div>
          <div className="mb-1 flex items-baseline justify-between text-xs">
            <span className="text-muted-foreground">Capacidad del volumen de archivo</span>
            <span className="font-mono tabular-nums text-muted-foreground">
              {disk && "error" in disk
                ? `estado no disponible: ${disk.error}`
                : `${fmtBytes(freeBytes)} libres de ${fmtBytes(totalBytes)} · ${usedPct ?? "—"}% usado`}
            </span>
          </div>
          <Progress
            value={usedPct ?? 0}
            aria-label="Uso del volumen de archivo"
            className={usedPct !== null && usedPct >= 90 ? "[&>div]:bg-destructive" : ""}
          />
        </div>

        {/* Automatic mode */}
        <div className="flex items-center justify-between gap-3 rounded-md border px-3 py-2">
          <div className="text-xs">
            <div className="font-medium">Modo automático (cron nocturno 04:17 UTC)</div>
            <div className="text-muted-foreground">
              Archiva cada rango a .zst ANTES de purgarlo. Sin archivo exitoso, esa tabla no se purga esa noche.
            </div>
          </div>
          <Switch
            checked={status?.auto_mode?.enabled ?? false}
            disabled={busy || !status}
            onCheckedChange={onToggleAuto}
            aria-label="Alternar modo automático de archivo"
          />
        </div>

        {/* Per-table manual export */}
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b text-left text-muted-foreground">
                <th className="py-1.5 pr-3 font-medium">Tabla</th>
                <th className="py-1.5 pr-3 font-medium">Ventana</th>
                <th className="py-1.5 pr-3 font-medium">Filas {" > "} ventana</th>
                <th className="py-1.5 pr-3 font-medium">Exportar</th>
              </tr>
            </thead>
            <tbody>
              {(status?.tables ?? []).map((t) => (
                <tr key={t.table} className="border-b last:border-0">
                  <td className="py-1.5 pr-3 font-mono">{t.table}</td>
                  <td className="py-1.5 pr-3 font-mono tabular-nums">{t.window_days}d</td>
                  <td className="py-1.5 pr-3 font-mono tabular-nums">
                    {t.rows_beyond_window === null ? "—" : t.rows_beyond_window.toLocaleString("en-US")}
                  </td>
                  <td className="py-1.5 pr-3">
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={
                        busy ||
                        !!exportRunning ||
                        t.rows_beyond_window === 0
                      }
                      onClick={() => void onExport(t.table)}
                    >
                      .csv.gz
                    </Button>
                  </td>
                </tr>
              ))}
              {!status && (
                <tr>
                  <td colSpan={4} className="py-3 text-muted-foreground">
                    Cargando estado de archivo…
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        {/* Existing archive files */}
        <div>
          <div className="mb-1 flex items-baseline justify-between text-xs">
            <span className="text-muted-foreground">
              Archivos existentes (rsync off-VPS: docs/RETENTION_POLICY.md)
            </span>
            <span className="font-mono tabular-nums text-muted-foreground">
              total {fmtBytes(status?.archives?.total_bytes ?? null)}
            </span>
          </div>
          {(status?.archives?.files ?? []).length === 0 ? (
            <div className="text-xs text-muted-foreground">Sin archivos aún.</div>
          ) : (
            <ul className="space-y-0.5 font-mono text-[11px] text-muted-foreground">
              {(status?.archives?.files ?? []).slice(0, 8).map((f) => (
                <li key={`${f.table}/${f.name}`} className="flex justify-between gap-2">
                  <span className="truncate">{f.table}/{f.name}</span>
                  <span className="shrink-0 tabular-nums">
                    {fmtBytes(f.bytes)} · {fmtWhen(f.modified_at)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* Action feedback + honest errors */}
        {actionMsg && <div className="text-xs text-muted-foreground">{actionMsg}</div>}
        {error && (
          <div className="rounded-md border border-destructive/50 px-3 py-2 font-mono text-[11px] text-destructive">
            {error}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
