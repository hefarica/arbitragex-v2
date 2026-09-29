"use client";

/**
 * QUANT LAYERS — el libro cuantitativo del Excel, como pantalla viva.
 *
 * Capas que se pintan (mismas hojas, mismos nombres):
 *   05_EDGES      F_e = spot/fair , w = −LN(F_e)   → columnas de la pata
 *   06_ROUTES     Σw , bound vinculante , sizing   → columnas de la ruta
 *   07_LEGS       cadena medida pata a pata        → panel expandido
 *   08_ROUTE_PNL  escalera de costes + net_bps     → grilla principal
 *   09_DASHBOARD  embudo por hops + top            → KPIs
 *
 * Reglas que NO se negocian aquí:
 *   R8/RULE 00 — `null` es NO COMPUTADO y se dibuja "—". Nunca 0, nunca "NaN".
 *   La capa NO declara EXECUTE: su techo es READY_TO_SIMULATE (firmar y emitir es
 *   del terminus de ejecución).
 *   El corte por defecto muestra las filas CON cifras; las declaradas no
 *   computadas se añaden con el toggle y siempre llevan su razón.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle, Download, RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { getPublicEdgeBaseUrl } from "@/lib/api-client";
import {
  buildGrid,
  fetchQuantLayers,
  fmtBps,
  fmtNum,
  fmtPct,
  fmtUsd,
  fmtWeight,
  funnelSteps,
  reasonBreakdown,
  shortAddress,
  shortKey,
  splitGrid,
  statusVariant,
  verdictVariant,
  windowReadout,
  type QuantGridRow,
  type QuantSnapshot,
} from "@/lib/quant-layers";

const WINDOWS: Array<{ minutes: number; label: string }> = [
  { minutes: 15, label: "15 min" },
  { minutes: 60, label: "1 h" },
  { minutes: 240, label: "4 h" },
  { minutes: 1440, label: "24 h" },
];

const REFRESH_MS = 15_000;

function Metric({
  label,
  value,
  hint,
  tone = "default",
  testId,
}: {
  label: string;
  value: React.ReactNode;
  hint?: string;
  tone?: "default" | "success" | "warning" | "danger";
  testId?: string;
}) {
  const toneClass =
    tone === "success"
      ? "text-success"
      : tone === "warning"
        ? "text-warning"
        : tone === "danger"
          ? "text-destructive"
          : "text-foreground";
  return (
    <div
      data-slot="card"
      data-testid={testId}
      className="p-4 rounded-2xl border border-border bg-card text-card-foreground"
    >
      <p className="text-[11px] uppercase tracking-wide text-muted-foreground">{label}</p>
      <p className={`text-2xl font-bold tabular-nums ${toneClass}`}>{value}</p>
      {hint ? <p className="text-[11px] text-muted-foreground mt-1">{hint}</p> : null}
    </div>
  );
}

function Th({ children, right = false }: { children: React.ReactNode; right?: boolean }) {
  return (
    <th
      className={`px-2 py-2 text-[11px] uppercase tracking-wide text-muted-foreground font-semibold whitespace-nowrap ${
        right ? "text-right" : "text-left"
      }`}
    >
      {children}
    </th>
  );
}

function Td({
  children,
  right = false,
  mono = false,
  title,
}: {
  children: React.ReactNode;
  right?: boolean;
  mono?: boolean;
  title?: string;
}) {
  return (
    <td
      title={title}
      className={`px-2 py-1.5 text-xs align-top whitespace-nowrap ${right ? "text-right" : "text-left"} ${
        mono ? "font-mono tabular-nums" : ""
      }`}
    >
      {children}
    </td>
  );
}

function LegPanel({ row }: { row: QuantGridRow }) {
  return (
    <div className="p-3 bg-muted/30 border-t border-border">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-muted-foreground mb-2">
        <span>
          07_LEGS · cadena medida · <span className="font-mono">{row.hops}</span> patas
        </span>
        <span>
          Σw = <span className="font-mono text-foreground">{fmtWeight(row.sumW)}</span>
        </span>
        <span>
          retorno de descubrimiento e^(−Σw)−1 ={" "}
          <span className="font-mono text-foreground">{fmtPct(row.discoveryReturnPct)}</span>
        </span>
        <span>
          bound vinculante = <span className="font-mono text-foreground">{fmtUsd(row.bindingBoundUsd)}</span>
        </span>
        <span>
          sizing = <span className="font-mono text-foreground">{fmtUsd(row.sizingUsd)}</span>
        </span>
        <span>
          cadena fair = <span className="font-mono text-foreground">{fmtUsd(row.fairChainUsd)}</span>
        </span>
        <span>
          bloque de cotización ={" "}
          <span className="font-mono text-foreground">
            {row.quoteBlock == null ? "no publicado" : row.quoteBlock.toLocaleString("en-US")}
          </span>
        </span>
      </div>
      <table className="w-full border-collapse">
        <thead>
          <tr className="border-b border-border">
            <Th>#</Th>
            <Th>Pata</Th>
            <Th>Pool</Th>
            <Th>DEX</Th>
            <Th right>AmountIn</Th>
            <Th right>AmountOut</Th>
            <Th right>spot</Th>
            <Th right>fair</Th>
            <Th right>F_e</Th>
            <Th right>w</Th>
            <Th right>bound USD</Th>
          </tr>
        </thead>
        <tbody>
          {row.legs.map((l) => (
            <tr key={`${row.routeKey}-leg-${l.legIndex}`} className="border-b border-border/50">
              <Td mono>{l.legIndex}</Td>
              <Td mono title={`${l.tokenIn} → ${l.tokenOut}`}>
                {shortAddress(l.tokenIn)} → {shortAddress(l.tokenOut)}
              </Td>
              <Td mono title={l.poolAddress}>
                {l.poolAddress ? shortAddress(l.poolAddress) : "—"}
              </Td>
              <Td>
                {l.dex ? (
                  <Badge variant="outline" className="text-[10px]">
                    {l.dex}
                    {l.poolType ? ` · ${l.poolType}` : ""}
                  </Badge>
                ) : (
                  "—"
                )}
              </Td>
              <Td right mono>
                {fmtNum(l.amountIn, 6)}
              </Td>
              <Td right mono>
                {fmtNum(l.amountOut, 6)}
              </Td>
              <Td right mono>
                {fmtNum(l.spot, 6)}
              </Td>
              <Td right mono>
                {fmtNum(l.fair, 6)}
              </Td>
              {/* F_e lleva 6 decimales: en pools de 1 bp el factor es indistinguible
                  de 1.000000 a 4 decimales y la señal se perdería en pantalla. */}
              <Td right mono>
                {fmtNum(l.factor, 6)}
              </Td>
              <Td right mono>
                {fmtWeight(l.weight)}
              </Td>
              <Td right mono title={l.boundReason ?? undefined}>
                {l.boundUsd == null ? `— (${l.boundReason ?? "no computado"})` : fmtUsd(l.boundUsd)}
              </Td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="text-[10px] text-muted-foreground mt-2">
        fee_included_in_spot = true: el spot sale de amount_in/amount_out MEDIDOS (el AMM ya cobró el fee), por eso
        F_e = spot/fair sin volver a descontar (1 − fee).
      </p>
    </div>
  );
}

export default function QuantClient({
  initialSnapshot,
  initialWindowMinutes,
  initialShowNoComputado,
}: {
  initialSnapshot: QuantSnapshot;
  initialWindowMinutes: number;
  initialShowNoComputado: boolean;
}) {
  const [snapshot, setSnapshot] = useState<QuantSnapshot>(initialSnapshot);
  const [windowMinutes, setWindowMinutes] = useState<number>(initialWindowMinutes);
  const [showNoComputado, setShowNoComputado] = useState<boolean>(initialShowNoComputado);
  const [autoRefresh, setAutoRefresh] = useState<boolean>(true);
  const [loading, setLoading] = useState<boolean>(false);
  const [expanded, setExpanded] = useState<string | null>(null);
  const inflight = useRef<AbortController | null>(null);

  const load = useCallback(
    async (minutes: number) => {
      inflight.current?.abort();
      const ac = new AbortController();
      inflight.current = ac;
      setLoading(true);
      const next = await fetchQuantLayers(getPublicEdgeBaseUrl(), {
        windowMinutes: minutes,
        limit: 500,
        top: 10,
        signal: ac.signal,
      });
      if (ac.signal.aborted) return;
      setSnapshot(next);
      setLoading(false);
    },
    [],
  );

  // Poll: vive y muere con el componente (una sola fuente de verdad en pantalla).
  useEffect(() => {
    if (!autoRefresh) return;
    const id = window.setInterval(() => {
      void load(windowMinutes);
    }, REFRESH_MS);
    return () => {
      window.clearInterval(id);
      inflight.current?.abort();
    };
  }, [autoRefresh, windowMinutes, load]);

  const data = snapshot.ok ? snapshot.data : null;

  const rows = useMemo(
    () => (data ? buildGrid(data.layers.routes, data.layers.pnl) : []),
    [data],
  );
  const { withFigures, noFigures } = useMemo(() => splitGrid(rows), [rows]);
  const readout = useMemo(() => windowReadout(rows), [rows]);
  const grid = showNoComputado ? rows : withFigures;
  const reasons = useMemo(() => (data ? reasonBreakdown(data.not_computed) : []), [data]);

  const pickWindow = (minutes: number) => {
    setWindowMinutes(minutes);
    if (typeof window !== "undefined") {
      const url = new URL(window.location.href);
      url.searchParams.set("window_minutes", String(minutes));
      window.history.replaceState(null, "", url.toString());
    }
    void load(minutes);
  };

  const toggleNoComputado = () => {
    const next = !showNoComputado;
    setShowNoComputado(next);
    if (typeof window !== "undefined") {
      const url = new URL(window.location.href);
      if (next) url.searchParams.set("nocomputado", "1");
      else url.searchParams.delete("nocomputado");
      window.history.replaceState(null, "", url.toString());
    }
  };

  return (
    <div className="p-4 md:p-6 min-h-screen text-foreground space-y-5">
      {/* ── Cabecera ─────────────────────────────────────────────────────── */}
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-2xl md:text-3xl font-bold tracking-tight">
            Quant layers — el libro Excel, vivo
          </h1>
          <p className="text-xs text-muted-foreground mt-1">
            TOKENS → POOLS → EDGES → ROUTES → LEGS → ROUTE_PNL → DASHBOARD sobre las detecciones{" "}
            <span className="text-foreground">medidas</span> de la ventana. Esta capa no firma ni emite: su techo es{" "}
            <span className="font-mono">READY_TO_SIMULATE</span>.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant={autoRefresh ? "success" : "secondary"} data-testid="quant-live-badge">
            {autoRefresh ? "AUTO 15 s" : "PAUSADO"}
          </Badge>
          <span className="text-[11px] text-muted-foreground" data-testid="quant-updated-at">
            {snapshot.fetchedAt ? `actualizado ${new Date(snapshot.fetchedAt).toLocaleTimeString()}` : "sin lectura"}
          </span>
        </div>
      </div>

      {/* ── Controles ────────────────────────────────────────────────────── */}
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-[11px] uppercase tracking-wide text-muted-foreground">Ventana</span>
        {WINDOWS.map((w) => (
          <button
            key={w.minutes}
            type="button"
            data-testid={`quant-window-${w.minutes}`}
            onClick={() => pickWindow(w.minutes)}
            className={`px-3 py-1 rounded-full border text-xs transition-colors ${
              windowMinutes === w.minutes
                ? "border-primary bg-primary/15 text-foreground"
                : "border-border bg-card text-muted-foreground hover:text-foreground"
            }`}
          >
            {w.label}
          </button>
        ))}
        <button
          type="button"
          data-testid="quant-refresh"
          onClick={() => void load(windowMinutes)}
          className="px-3 py-1 rounded-full border border-border bg-card text-xs text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5"
        >
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
          Actualizar
        </button>
        <button
          type="button"
          data-testid="quant-toggle-autorefresh"
          onClick={() => setAutoRefresh((v) => !v)}
          className="px-3 py-1 rounded-full border border-border bg-card text-xs text-muted-foreground hover:text-foreground"
        >
          {autoRefresh ? "Pausar auto" : "Reanudar auto"}
        </button>
      </div>

      {/* ── Falla honesta del endpoint ───────────────────────────────────── */}
      {!snapshot.ok ? (
        <div
          data-testid="quant-error"
          className="p-4 rounded-2xl border border-destructive/40 bg-destructive/10 text-destructive text-sm"
        >
          <p className="font-semibold flex items-center gap-2">
            <AlertTriangle size={16} /> /api/quant/layers no respondió con datos
          </p>
          <p className="mt-1 font-mono text-xs break-all">{snapshot.error}</p>
          <p className="mt-1 text-xs opacity-80">
            No se sustituye por ceros ni por una tabla vacía: sin medición no hay capa cuantitativa.
          </p>
        </div>
      ) : null}

      {data ? (
        <>
          {/* ── 09_DASHBOARD · embudo ─────────────────────────────────────── */}
          <div data-testid="quant-funnel" className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
            {funnelSteps(data.layers.dashboard).map((s) => (
              <Metric
                key={s.key}
                testId={`quant-funnel-${s.key}`}
                label={s.label}
                value={s.value.toLocaleString("en-US")}
                hint={s.of == null ? "universo de la ventana" : `de ${s.of.toLocaleString("en-US")} anteriores`}
              />
            ))}
          </div>

          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
            <Metric
              testId="quant-with-figures"
              label="Con cifras"
              value={readout.withFigures.toLocaleString("en-US")}
              hint="net_bps computado"
            />
            <Metric
              testId="quant-execute"
              label="EJECUTAR (≥2 %)"
              value={readout.execute.toLocaleString("en-US")}
              tone={readout.execute > 0 ? "success" : "default"}
              hint="listo para simular"
            />
            <Metric
              testId="quant-marginal"
              label="MARGINAL"
              value={readout.marginal.toLocaleString("en-US")}
              tone={readout.marginal > 0 ? "warning" : "default"}
              hint="0 < net_bps < 200"
            />
            <Metric
              testId="quant-reject"
              label="RECHAZAR"
              value={readout.reject.toLocaleString("en-US")}
              hint="coste > gross"
            />
            <Metric
              testId="quant-nofigures"
              label="Sin cifras"
              value={readout.noViable.toLocaleString("en-US")}
              hint="no computado, con razón"
            />
            <Metric
              testId="quant-best-net"
              label="Mejor net_bps"
              value={fmtBps(readout.bestNetBps)}
              tone={
                readout.bestNetBps == null
                  ? "default"
                  : readout.bestNetBps >= 200
                    ? "success"
                    : readout.bestNetBps > 0
                      ? "warning"
                      : "danger"
              }
              hint={data.layers.dashboard.bestRouteKey ? shortKey(data.layers.dashboard.bestRouteKey, 26) : "sin ruta"}
            />
          </div>

          {/* ── 09_DASHBOARD · embudo por hops ───────────────────────────── */}
          <div data-testid="quant-by-hops" className="rounded-2xl border border-border bg-card p-4">
            <h2 className="text-sm font-semibold mb-2">Embudo por patas (hops)</h2>
            <table className="w-full border-collapse">
              <thead>
                <tr className="border-b border-border">
                  <Th>Hops</Th>
                  <Th right>Rutas</Th>
                  <Th right>Con señal</Th>
                  <Th right>Viables</Th>
                  <Th right>Listas p/ simular</Th>
                </tr>
              </thead>
              <tbody>
                {data.layers.dashboard.byHops.length === 0 ? (
                  <tr>
                    <Td>—</Td>
                    <Td right>—</Td>
                    <Td right>—</Td>
                    <Td right>—</Td>
                    <Td right>—</Td>
                  </tr>
                ) : (
                  data.layers.dashboard.byHops.map((h) => (
                    <tr key={h.hops} className="border-b border-border/50" data-testid={`quant-hops-${h.hops}`}>
                      <Td mono>{h.hops}</Td>
                      <Td right mono>
                        {h.routes}
                      </Td>
                      <Td right mono>
                        {h.signal}
                      </Td>
                      <Td right mono>
                        {h.viable}
                      </Td>
                      <Td right mono>
                        {h.executable}
                      </Td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>

          {/* ── 08_ROUTE_PNL · grilla principal ─────────────────────────── */}
          <div className="rounded-2xl border border-border bg-card">
            <div className="flex flex-wrap items-center justify-between gap-2 p-4 border-b border-border">
              <div>
                <h2 className="text-sm font-semibold">Ledger de rutas · 06_ROUTES ⨝ 08_ROUTE_PNL</h2>
                <p className="text-[11px] text-muted-foreground">
                  <span className="font-mono text-foreground">{withFigures.length}</span> con cifras ·{" "}
                  <span className="font-mono text-foreground">{noFigures.length}</span> declaradas no computadas ·
                  mostrando <span className="font-mono text-foreground">{grid.length}</span> filas
                </p>
                {/* QUANT-PNL-01: dos "net" conviven en la dapp y NO son el mismo
                    número. Decirlo aquí evita que parezca una contradicción. */}
                <p className="text-[11px] text-muted-foreground mt-1">
                  <span className="text-foreground">Net USD / Net bps</span> de esta tabla = gross{" "}
                  <span className="text-foreground">medido</span> − escalera de costes del modelo (gas base + por
                  pata, flash, tip, haircut). El net del feed de tarjetas es la medición del searcher, que ya lleva su
                  propio gas: son dos cuentas distintas, cada una con su procedencia. La desviación contra la cadena{" "}
                  <span className="font-mono">fair</span> se publica aparte y no se suma (ya vive dentro del gross
                  medido).
                </p>
              </div>
              <button
                type="button"
                data-testid="quant-toggle-nocomputado"
                onClick={toggleNoComputado}
                className={`px-3 py-1 rounded-full border text-xs ${
                  showNoComputado
                    ? "border-primary bg-primary/15 text-foreground"
                    : "border-border bg-card text-muted-foreground hover:text-foreground"
                }`}
              >
                {showNoComputado ? "Ocultar no computadas" : `Mostrar no computadas (${noFigures.length})`}
              </button>
            </div>

            {grid.length === 0 ? (
              <p className="p-6 text-sm text-muted-foreground" data-testid="quant-grid-empty">
                Ninguna ruta de la ventana tiene <span className="font-mono">net_bps</span> computado. No se rellena con
                ceros: revisa la ventana o el panel de no computadas.
              </p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full border-collapse" data-testid="quant-grid">
                  <thead>
                    <tr className="border-b border-border">
                      <Th>Ruta</Th>
                      <Th right>Hops</Th>
                      <Th>Estado</Th>
                      <Th>Veredicto</Th>
                      <Th right>Σw</Th>
                      <Th right>e^(−Σw)−1</Th>
                      <Th right>Bound USD</Th>
                      <Th right>Sizing</Th>
                      <Th right>Final</Th>
                      <Th right>Gross</Th>
                      <Th right>Gross bps</Th>
                      <Th right>Desv. fair</Th>
                      <Th right>Costes</Th>
                      <Th right>Net USD</Th>
                      <Th right>Net bps</Th>
                      <Th>Razón</Th>
                    </tr>
                  </thead>
                  <tbody>
                    {grid.map((r) => (
                      <React.Fragment key={r.routeKey}>
                        <tr
                          data-testid={`quant-row-${r.routeKey}`}
                          className={`border-b border-border/50 hover:bg-muted/30 cursor-pointer ${
                            r.netBps == null ? "opacity-70" : ""
                          }`}
                          onClick={() => setExpanded((cur) => (cur === r.routeKey ? null : r.routeKey))}
                        >
                          <Td mono title={r.routeKey}>
                            {shortKey(r.routeKey)}
                          </Td>
                          <Td right mono>
                            {r.hops}
                          </Td>
                          <Td>
                            <Badge variant={statusVariant(r.status)} className="text-[10px]">
                              {r.status}
                            </Badge>
                          </Td>
                          <Td>
                            <Badge variant={verdictVariant(r.verdict)} className="text-[10px]">
                              {r.verdict}
                            </Badge>
                          </Td>
                          <Td right mono>
                            {fmtWeight(r.sumW)}
                          </Td>
                          <Td right mono>
                            {fmtPct(r.discoveryReturnPct)}
                          </Td>
                          <Td right mono>
                            {fmtUsd(r.bindingBoundUsd)}
                          </Td>
                          <Td right mono>
                            {fmtUsd(r.sizingUsd)}
                          </Td>
                          <Td right mono>
                            {fmtUsd(r.finalUsd)}
                          </Td>
                          <Td right mono>
                            {fmtUsd(r.grossUsd)}
                          </Td>
                          <Td right mono>
                            {fmtBps(r.grossBps)}
                          </Td>
                          {/* QUANT-PNL-01: diagnóstico publicado y visible — el
                              impacto ya vive dentro del gross, así que esta
                              columna NO se suma a los costes. */}
                          <Td right mono title="desviación de la cadena medida contra la cadena fair (diagnóstico, no coste)">
                            {fmtUsd(r.deviationVsFairUsd)}
                          </Td>
                          <Td right mono>
                            {fmtUsd(r.totalCostUsd)}
                          </Td>
                          <Td
                            right
                            mono
                            title={r.netUsd != null && r.netUsd < 0 ? "pérdida medida" : undefined}
                          >
                            <span className={r.netUsd != null && r.netUsd < 0 ? "text-destructive" : ""}>
                              {fmtUsd(r.netUsd)}
                            </span>
                          </Td>
                          <Td right mono>
                            <span
                              className={
                                r.netBps == null
                                  ? "text-muted-foreground"
                                  : r.netBps >= 200
                                    ? "text-success"
                                    : r.netBps > 0
                                      ? "text-warning"
                                      : "text-destructive"
                              }
                            >
                              {fmtBps(r.netBps)}
                            </span>
                          </Td>
                          <Td>
                            <span className="text-[11px] text-muted-foreground">{r.whyNot ?? "—"}</span>
                          </Td>
                        </tr>
                        {expanded === r.routeKey ? (
                          <tr data-testid={`quant-detail-${r.routeKey}`}>
                            <td colSpan={16} className="p-0">
                              <LegPanel row={r} />
                            </td>
                          </tr>
                        ) : null}
                      </React.Fragment>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          {/* ── Top por net_bps ─────────────────────────────────────────── */}
          <div data-testid="quant-top" className="rounded-2xl border border-border bg-card p-4">
            <h2 className="text-sm font-semibold mb-2">
              Top {data.layers.dashboard.top.length} por net_bps (de {data.layers.dashboard.routes} rutas)
            </h2>
            <table className="w-full border-collapse">
              <thead>
                <tr className="border-b border-border">
                  <Th>#</Th>
                  <Th>Ruta</Th>
                  <Th right>Hops</Th>
                  <Th right>Net bps</Th>
                  <Th right>Net USD</Th>
                  <Th>Veredicto</Th>
                </tr>
              </thead>
              <tbody>
                {data.layers.dashboard.top.length === 0 ? (
                  <tr>
                    <td className="px-2 py-3 text-xs text-muted-foreground" colSpan={6}>
                      Ninguna ruta con net_bps computado en esta ventana.
                    </td>
                  </tr>
                ) : (
                  data.layers.dashboard.top.map((t, i) => (
                    <tr key={`${t.routeKey}-${i}`} className="border-b border-border/50">
                      <Td mono>{i + 1}</Td>
                      <Td mono title={t.routeKey}>
                        {shortKey(t.routeKey)}
                      </Td>
                      <Td right mono>
                        {t.hops}
                      </Td>
                      <Td right mono>
                        {fmtBps(t.netBps)}
                      </Td>
                      <Td right mono>
                        {fmtUsd(t.netUsd)}
                      </Td>
                      <Td>
                        <Badge variant={verdictVariant(t.verdict)} className="text-[10px]">
                          {t.verdict}
                        </Badge>
                      </Td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>

          {/* ── R8 explícito: lo no computado, con su razón ─────────────── */}
          <div data-testid="quant-notcomputed" className="rounded-2xl border border-border bg-card p-4">
            <h2 className="text-sm font-semibold mb-1">
              No computado en la ventana:{" "}
              <span className="font-mono text-foreground">{data.not_computed_count}</span> filas
            </h2>
            <p className="text-[11px] text-muted-foreground mb-2">
              {data.rows_in_window} filas medidas leídas · se listan hasta 50 razones. Una fila sin cadena medida no
              entra en la capa: se declara, no se rellena.
            </p>
            {reasons.length === 0 ? (
              <p className="text-xs text-muted-foreground">Sin filas no computadas.</p>
            ) : (
              <ul className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-1">
                {reasons.map((r) => (
                  <li key={r.reason} className="flex items-center justify-between text-xs">
                    <span className="font-mono">{r.reason}</span>
                    <span className="font-mono text-muted-foreground">{r.count}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>

          {/* ── Notas del contrato + config viva ────────────────────────── */}
          <div data-testid="quant-notes" className="rounded-2xl border border-border bg-card p-4 space-y-3">
            <div>
              <h2 className="text-sm font-semibold mb-1">Notas del contrato (verbatim del endpoint)</h2>
              <ul className="list-disc pl-5 space-y-0.5">
                {data.notes.map((n) => (
                  <li key={n} className="text-[11px] text-muted-foreground">
                    {n}
                  </li>
                ))}
              </ul>
            </div>
            <div className="text-[11px] text-muted-foreground">
              <span className="uppercase tracking-wide">Config viva del sizing y costes</span>
              <div className="mt-1 grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-x-6 gap-y-1 font-mono">
                <span>slippage {data.config.slippageBps} bps</span>
                <span>capital ${data.config.capitalUsd}</span>
                <span>util {data.config.utilizationCap}</span>
                <span>gas base ${data.config.gasBaseUsd}</span>
                <span>gas/hop ${data.config.gasPerHopUsd}</span>
                <span>flash {data.config.flashBps} bps</span>
                <span>tip {data.config.tipBps} bps</span>
                <span>haircut {data.config.riskHaircutBps} bps</span>
                <span>dust ${data.config.dustUsd}</span>
                <span>target {data.config.targetNetBps} bps</span>
                <span>min bound ${data.config.minBoundUsd}</span>
                <span>ventana {data.window_minutes} min</span>
              </div>
            </div>
            <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
              <Download size={13} />
              <a
                className="underline hover:text-foreground"
                href={`${getPublicEdgeBaseUrl()}/api/quant/layers?window_minutes=${data.window_minutes}&limit=500&top=10`}
                target="_blank"
                rel="noreferrer"
              >
                abrir el JSON crudo de esta ventana
              </a>
              <span>· generado {new Date(data.generated_at).toLocaleString()}</span>
            </div>
          </div>
        </>
      ) : null}
    </div>
  );
}
