"use client";

import { useState, useId } from "react";
import { PlayIcon, ChevronDownIcon, LoaderIcon, CheckCircleIcon, XCircleIcon } from "lucide-react";

import { cn } from "@/lib/utils";

/**
 * G-SIM-1 PR-B2b Fase 5 — Simulate button with route_source selector.
 *
 * Sends POST /api/v1/opportunities/:id/simulate with the selected enrichment
 * path (A1 pg_metadata | A2 searcher_api | A3 simctl_lookup). Displays inline
 * loading/success/error state so the operator sees the result without leaving
 * the opportunity detail dialog.
 *
 * R8 fail-honest: shows the real error from the API (never fabricates success).
 */

type RouteSource = "pg_metadata" | "searcher_api" | "simctl_lookup";

const ROUTE_SOURCES: { value: RouteSource; label: string; short: string; description: string }[] = [
  {
    value: "pg_metadata",
    label: "PG route_metadata (A1)",
    short: "PG (A1)",
    description: "Persistent route topology from the opportunities table. Source of truth.",
  },
  {
    value: "searcher_api",
    label: "Searcher API (A2)",
    short: "API (A2)",
    description: "Live in-memory route from searcher-rs. Lowest latency for fresh opps.",
  },
  {
    value: "simctl_lookup",
    label: "sim-ctl Lookup (A3)",
    short: "sim-ctl (A3)",
    description: "sim-ctl queries PG autonomously. No upstream dependency.",
  },
];

type SimState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "success"; detail: string }
  | { kind: "error"; detail: string };

interface Props {
  opportunityId: string;
  /** Base URL for the api-server. Defaults to the standard edge path. */
  apiBase?: string;
  /** Compact mode: renders as a single button with dropdown (for tight layouts). */
  compact?: boolean;
}

export function SimulateButton({ opportunityId, compact = false }: Props) {
  const [routeSource, setRouteSource] = useState<RouteSource>("simctl_lookup");
  const [state, setState] = useState<SimState>({ kind: "idle" });
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const listboxId = useId();

  const selectedSource = ROUTE_SOURCES.find((r) => r.value === routeSource)!;

  async function handleSimulate() {
    setState({ kind: "loading" });
    setDropdownOpen(false);
    try {
      const resp = await fetch(
        `/api/v1/opportunities/${encodeURIComponent(opportunityId)}/simulate`,
        {
          method: "POST",
          headers: { "content-type": "application/json", accept: "application/json" },
          body: JSON.stringify({ route_source: routeSource }),
        },
      );
      const text = await resp.text();
      let parsed: unknown;
      try {
        parsed = text ? JSON.parse(text) : {};
      } catch {
        parsed = { raw: text };
      }

      if (resp.ok) {
        const result = (parsed as { result?: { passed?: boolean; fail_reason?: string } }).result;
        const detail = result?.passed
          ? "Simulation passed (net profit > 0)"
          : (result?.fail_reason ?? `Simulation returned (status ${resp.status})`);
        setState({ kind: result?.passed ? "success" : "error", detail });
      } else {
        const errDetail =
          (parsed as { detail?: string; error?: string }).detail ??
          (parsed as { error?: string }).error ??
          `HTTP ${resp.status}`;
        setState({ kind: "error", detail: errDetail });
      }
    } catch (e) {
      setState({ kind: "error", detail: (e as Error).message });
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        {/* Route source selector dropdown */}
        <div className="relative">
          <button
            type="button"
            onClick={() => setDropdownOpen((v) => !v)}
            aria-haspopup="listbox"
            aria-expanded={dropdownOpen}
            aria-controls={listboxId}
            className={cn(
              "inline-flex h-9 items-center gap-1.5 rounded-md border border-border bg-card/60 px-3 text-xs font-medium backdrop-blur-sm",
              "hover:bg-accent/60 transition-colors",
            )}
            title={selectedSource.description}
          >
            <span className="text-muted-foreground">{selectedSource.short}</span>
            <ChevronDownIcon className="size-3 text-muted-foreground" />
          </button>
          {dropdownOpen && (
            <>
              {/* Click-outside overlay */}
              <div
                className="fixed inset-0 z-40"
                onClick={() => setDropdownOpen(false)}
                aria-hidden
              />
              <ul
                id={listboxId}
                role="listbox"
                className="absolute bottom-full left-0 z-50 mb-1 w-72 overflow-hidden rounded-lg border border-border bg-popover/95 backdrop-blur-xl shadow-xl"
              >
                {ROUTE_SOURCES.map((src) => (
                  <li key={src.value} role="option" aria-selected={src.value === routeSource}>
                    <button
                      type="button"
                      onClick={() => {
                        setRouteSource(src.value);
                        setDropdownOpen(false);
                      }}
                      className={cn(
                        "flex w-full flex-col gap-0.5 px-3 py-2.5 text-left transition-colors",
                        src.value === routeSource
                          ? "bg-primary/10 text-foreground"
                          : "hover:bg-accent/60 text-foreground/80",
                      )}
                    >
                      <span className="text-xs font-semibold">{src.label}</span>
                      <span className="text-[11px] leading-relaxed text-muted-foreground">
                        {src.description}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>

        {/* Simulate action button */}
        <button
          type="button"
          onClick={handleSimulate}
          disabled={state.kind === "loading"}
          className={cn(
            "inline-flex h-9 items-center gap-1.5 rounded-md px-4 text-xs font-semibold transition-all",
            "bg-primary text-primary-foreground shadow-sm",
            "hover:bg-primary/90 hover:shadow-md active:scale-[0.98]",
            "disabled:cursor-not-allowed disabled:opacity-60 disabled:active:scale-100",
          )}
        >
          {state.kind === "loading" ? (
            <LoaderIcon className="size-3.5 animate-spin" />
          ) : (
            <PlayIcon className="size-3.5" />
          )}
          {compact ? "Sim" : "Simulate"}
        </button>
      </div>

      {/* Result / error display */}
      {state.kind === "success" && (
        <div className="flex items-start gap-2 rounded-md border border-success/30 bg-success/10 p-2.5 text-xs">
          <CheckCircleIcon className="mt-0.5 size-3.5 shrink-0 text-success" />
          <div>
            <p className="font-medium text-success">{selectedSource.label}</p>
            <p className="mt-0.5 text-foreground/80">{state.detail}</p>
          </div>
        </div>
      )}
      {state.kind === "error" && (
        <div className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-2.5 text-xs">
          <XCircleIcon className="mt-0.5 size-3.5 shrink-0 text-destructive" />
          <div>
            <p className="font-medium text-destructive">
              {selectedSource.label} — {state.detail.includes("not_implemented") || state.detail.includes("pending") ? "Not yet wired" : "Failed"}
            </p>
            <p className="mt-0.5 break-words font-mono text-[11px] text-foreground/70">
              {state.detail}
            </p>
          </div>
          {/* Fail-honest hint: this is expected until B2c wires the encoder */}
          {state.detail.includes("not_implemented") || state.detail.includes("pending") || state.detail.includes("not yet wired") ? (
            <p className="mt-1 text-[10px] italic text-muted-foreground">
              Encoder wiring pending (B2c). The A1/A2/A3 enrichment path is the foundation.
            </p>
          ) : null}
        </div>
      )}
    </div>
  );
}
