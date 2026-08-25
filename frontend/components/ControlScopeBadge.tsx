/**
 * =============================================================================
 * ControlScopeBadge — FE-0041 (FE-MASTER §52/§53/§54/§63)
 * =============================================================================
 *
 * One honest label for every control surface, so the operator can tell at a
 * glance what a control can and cannot touch:
 *
 *   VIEW_ONLY       renders the wire; zero writes anywhere.
 *   LOCAL_PREFS     writes ONLY browser-local presentation state
 *                   (localStorage) — never the runtime (§53: /settings is
 *                   presentation scope by contract).
 *   RUNTIME_MUTATION writes through the config plane (putTradingConfig /
 *                   admin endpoints) and reaches searcher-rs via hot-reload —
 *                   action-machine territory (§56).
 *
 * Pure component, no hooks, no "use client" — renderable from Server
 * Components (config pages) and Client Components (tabs) alike. Every label
 * explains itself via title (§63 explainability).
 */

// SSR-test support (repo pattern): the node test transformer's classic JSX
// path needs the React namespace in module scope — inert for the Next
// automatic-runtime app build.
import * as React from "react";
import { Badge } from "@/components/ui/badge";

export type ControlScope = "VIEW_ONLY" | "LOCAL_PREFS" | "RUNTIME_MUTATION";

interface ScopeSpec {
  /** Visible label. */
  label: string;
  /** Badge variant (repo set only). */
  variant: "outline" | "info" | "warning";
  /** Explainability copy (§63) — what this control can and cannot write. */
  title: string;
}

const SCOPES: Record<ControlScope, ScopeSpec> = {
  VIEW_ONLY: {
    label: "VIEW_ONLY",
    variant: "outline",
    title:
      "Superficie de lectura: renderiza el wire tal cual; cero escrituras en cualquier capa.",
  },
  LOCAL_PREFS: {
    label: "LOCAL_PREFS",
    variant: "info",
    title:
      "Preferencias de presentación: escribe SOLO localStorage de este navegador; jamás muta runtime ni config (§53).",
  },
  RUNTIME_MUTATION: {
    label: "RUNTIME_MUTATION",
    variant: "warning",
    title:
      "Mutación de runtime: escribe por el plano de config (SSOT /config/trading vía putTradingConfig o endpoint admin) y llega al searcher vía hot-reload (§54 §56).",
  },
};

export function ControlScopeBadge({
  kind,
  className = "",
}: {
  kind: ControlScope;
  className?: string;
}): JSX.Element {
  const spec = SCOPES[kind];
  return (
    <Badge
      variant={spec.variant}
      title={spec.title}
      data-testid={`control-scope-${kind}`}
      className={`font-mono text-[10px] font-semibold uppercase tracking-wide ${className}`}
    >
      {spec.label}
    </Badge>
  );
}
