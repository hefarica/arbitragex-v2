"use client";

/**
 * =============================================================================
 * RegistryCoherenceStrip — FE-0040 (FE-MASTER P10-DRIFT-HOME §56/§57)
 * =============================================================================
 *
 * Total-coherence surface for one registry resource: the backend drift engine
 * (drift_observations, GET /api/system/drift via useOmniDrift) reports every
 * UNRESOLVED divergence between layers with its hash pair. This strip renders
 * exactly that wire:
 *
 *   0 unresolved observations + query genuinely answered → COHERENT
 *   ≥1 unresolved observation                            → DRIFT (+ layer chips,
 *                                                           hash_a ≠ hash_b rows)
 *   poll failed / backend answered `reason` (e.g. table absent) → NO COMPUTADO
 *
 * §79: the FE NEVER recomputes hashes or re-derives a verdict — presence vs
 * absence of engine observations IS the verdict. R8: an empty list caused by
 * `drift_observations_table_absent` renders NO COMPUTADO, never COHERENT.
 *
 * Layer chips: DB / Redis / Runtime / Frontend (the four canonical copies).
 * The wire carries divergences only — there is no continuous per-layer hash
 * snapshot — so a chip lights destructive when an observation names that
 * layer; otherwise it stays neutral ("sin divergencia reportada", NOT
 * "verificada"). The Frontend chip additionally shows the FE's own view
 * (rows seen + last refresh) as VALUES.
 *
 * Declared gap (nivel-(b)): useDriftDetection/DriftReport
 * (/api/v1/drift/status, snapshots[] with per-source config_hash) is NOT
 * emitted by the backend — mounted nowhere until it exists.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";
import { Badge } from "@/components/ui/badge";
import type { DriftObservation } from "@/lib/registries/types-omni";

// ---------------------------------------------------------------------------
// Pure helpers (exported for tests)
// ---------------------------------------------------------------------------

export type CoherenceVerdict =
  | { kind: "DRIFT"; count: number }
  | { kind: "COHERENT" }
  | { kind: "NOT_COMPUTED"; why: string };

/**
 * Derive the verdict from the wire state. Pure. Order matters: an HTTP-level
 * poll failure and a structural backend `reason` both block the COHERENT
 * claim (R8) — only a genuinely-answered empty list is coherence.
 */
export function coherenceVerdict(
  observations: readonly DriftObservation[],
  pollError: string | null,
  reason: string | null,
): CoherenceVerdict {
  if (pollError) return { kind: "NOT_COMPUTED", why: `poll_failed: ${pollError}` };
  if (observations.length > 0) return { kind: "DRIFT", count: observations.length };
  if (reason) return { kind: "NOT_COMPUTED", why: reason };
  return { kind: "COHERENT" };
}

/** Map a wire layer label to its canonical chip group (unknown → verbatim). */
export function layerGroup(layer: string): string {
  const l = layer.toLowerCase();
  if (["postgresql", "persistence", "pg", "db"].includes(l)) return "DB";
  if (["redis", "redis_pubsub"].includes(l)) return "Redis";
  if (["runtime", "searcher_rs", "arc_swap", "api"].includes(l)) return "Runtime";
  if (["frontend", "frontend_refresh"].includes(l)) return "Frontend";
  if (l === "toml") return "TOML";
  return layer;
}

/** 8…6 elision for 64-hex config hashes (full hash in the cell title). */
function shortHash(h: string | null): string {
  if (h == null) return "—";
  return h.length <= 16 ? h : `${h.slice(0, 8)}…${h.slice(-6)}`;
}

const CHIP_GROUPS = ["DB", "Redis", "Runtime", "Frontend"] as const;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface RegistryCoherenceStripProps {
  /** RegistryKey whose observations are passed in (display only). */
  resource: string;
  /** Unresolved drift observations for THIS resource (useOmniDrift.byResource). */
  observations: readonly DriftObservation[];
  /** HTTP-level poll failure from the hook → NO COMPUTADO. */
  pollError: string | null;
  /** Backend structural reason for an empty list (R8) → NO COMPUTADO. */
  reason: string | null;
  /** True only while the FIRST check is in flight (hook semantics). */
  loading: boolean;
  /** FE's own view of the resource: rows it currently holds; null = none. */
  frontendRows: number | null;
  /** ISO of the FE's last successful coherence poll (epoch-0 guard → null). */
  frontendRefreshedAt: string | null;
}

export function RegistryCoherenceStrip({
  resource,
  observations,
  pollError,
  reason,
  loading,
  frontendRows,
  frontendRefreshedAt,
}: RegistryCoherenceStripProps): JSX.Element {
  const verdict = coherenceVerdict(observations, pollError, reason);

  // Flag every layer group named by an observation; unknown layers get their
  // own verbatim chips so nothing the engine reports is dropped (§28).
  const flagged = new Set<string>();
  for (const o of observations) {
    flagged.add(layerGroup(o.layer_a));
    flagged.add(layerGroup(o.layer_b));
  }
  const extraGroups = [...flagged].filter(
    (g) => !(CHIP_GROUPS as readonly string[]).includes(g),
  );

  return (
    <div data-testid={`coherence-strip-${resource}`} className="space-y-3">
      {/* Verdict line */}
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm font-medium">Coherencia total (DB=Redis=Runtime=Frontend)</span>
        {loading && observations.length === 0 && !pollError && !reason ? (
          <Badge variant="secondary" data-testid="coherence-verdict">PRIMER CHEQUEO…</Badge>
        ) : verdict.kind === "COHERENT" ? (
          <Badge variant="outline" className="border-success/40 text-success" data-testid="coherence-verdict">
            COHERENT
          </Badge>
        ) : verdict.kind === "DRIFT" ? (
          <Badge variant="destructive" data-testid="coherence-verdict">
            DRIFT · {verdict.count}
          </Badge>
        ) : (
          <Badge variant="secondary" data-testid="coherence-verdict">
            NO COMPUTADO · {verdict.why}
          </Badge>
        )}
      </div>
      <p className="text-xs text-muted-foreground" data-testid="coherence-verdict-basis">
        {verdict.kind === "DRIFT"
          ? `El motor de drift reporta ${verdict.count} observación(es) sin resolver para «${resource}».`
          : verdict.kind === "COHERENT"
            ? `El motor de drift reporta 0 observaciones sin resolver para «${resource}» — el veredicto es del backend; el FE no recomputa hashes (§79).`
            : `Sin veredicto: ${verdict.why}. Ausencia de chequeo NO es COHERENT (R8).`}
      </p>

      {/* Layer chips: divergencia reportada vs sin divergencia reportada */}
      <div className="flex flex-wrap items-center gap-1.5" data-testid="coherence-chips">
        {CHIP_GROUPS.map((g) => {
          const isFlagged = flagged.has(g);
          const isFrontend = g === "Frontend";
          const label = isFrontend
            ? `Frontend · ${frontendRows == null ? "sin filas" : `${frontendRows} filas`} · ${
                frontendRefreshedAt == null ? "—" : "refrescado"
              }`
            : g;
          return (
            <span
              key={g}
              data-testid={`coherence-chip-${g}`}
              title={
                isFlagged
                  ? "Capa nombrada por ≥1 observación de drift sin resolver"
                  : isFrontend
                    ? "Vista local del FE (filas + refresco) — el FE no compara hashes (§79)"
                    : "Sin divergencia reportada por el motor (no es prueba de igualdad)"
              }
              className={`rounded-full border px-2 py-0.5 font-mono text-[10px] font-semibold uppercase tracking-wide ${
                isFlagged
                  ? "border-destructive/40 bg-destructive/10 text-destructive"
                  : "border-border bg-muted/50 text-muted-foreground"
              }`}
            >
              {label}
            </span>
          );
        })}
        {extraGroups.map((g) => (
          <span
            key={g}
            data-testid={`coherence-chip-${g}`}
            title="Capa no canónica nombrada por una observación — verbatim (§28)"
            className="rounded-full border border-destructive/40 bg-destructive/10 px-2 py-0.5 font-mono text-[10px] font-semibold uppercase tracking-wide text-destructive"
          >
            {g}
          </span>
        ))}
      </div>

      {/* Per-observation rows: the engine's divergences, verbatim */}
      {observations.length > 0 ? (
        <ul className="space-y-1" data-testid="coherence-observations">
          {observations.map((o, i) => (
            <li
              key={o.id}
              data-testid={`coherence-obs-${i}`}
              className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 rounded border border-border/60 px-2 py-1 font-mono text-xs"
            >
              <span
                className={
                  o.severity === "critical" || o.severity === "error"
                    ? "text-destructive"
                    : "text-warning"
                }
              >
                {o.severity}
              </span>
              <span className="text-foreground/80">
                {o.layer_a} ↔ {o.layer_b}
              </span>
              <span title={o.hash_a ?? undefined} className="text-muted-foreground">
                {shortHash(o.hash_a)}
              </span>
              <span aria-hidden className="text-muted-foreground/60">≠</span>
              <span title={o.hash_b ?? undefined} className="text-muted-foreground">
                {shortHash(o.hash_b)}
              </span>
              <span className="ml-auto text-muted-foreground">{o.diff_count} diffs</span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-xs italic text-muted-foreground/70" data-testid="coherence-no-rows">
          Sin observaciones — el wire expone sólo DIVERGENCIAS con su par de hashes;
          no existe snapshot continuo per-capa (nivel-(b)).
        </p>
      )}

      <p className="text-[11px] italic text-muted-foreground/70">
        DriftReport v2 (/api/v1/drift/status, useDriftDetection) no emitido por el
        backend (nivel-(b)) — este strip consume el wire real /api/system/drift.
      </p>
    </div>
  );
}
