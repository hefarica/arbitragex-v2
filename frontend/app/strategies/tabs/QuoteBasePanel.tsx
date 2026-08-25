/**
 * FE-MASTER · Quote/Base panel (FE-0013/0014/0015 — P4, workbook 05_QUOTE_BASE).
 *
 * Three surfaces over REAL runtime data (RULE 00 — every value renders what
 * the payload carries; §79 — the panel NEVER recomputes a score):
 *
 *   1. §8 anchor card  — current quote anchor: symbol, QuoteScore (0..100),
 *      quote_version / graph_version, the five score components and the
 *      runtime weight mirror (§9 — weights are a config mirror, Σ≈1 enforced
 *      backend-side).
 *   2. §9 explainable table — per-token components + score, payload order
 *      (backend-fixed: score desc, tie symbol asc → address asc).
 *   3. §10 preview-before-apply — admin-gated weight editor that POSTs the
 *      PROPOSED weights for a deterministic backend re-ranking. NEVER a
 *      mutation (QB-TOPOLOGY-01: graph_rebuild_required is a doctrine
 *      literal false; apply flows through the canonical knobs, not here).
 *
 * R8 honest states: the endpoint's 503s (quote_anchor_not_published /
 * quote_anchor_snapshot_corrupted / redis_unavailable) render verbatim;
 * `null`/absent renders "—", never 0.
 *
 * §11 (FE-0016): a coherencia line compares the canonical-knobs snapshot
 * (what the operator env-configured) against the runtime mirror the anchor
 * carries. Quote weights have NO runtime mutation path — the knobs are
 * env/boot-sourced, so ackEventId stays null (steady states only; nothing
 * fabricated). Cadence note (cont.42 addendum): the root ArbxRealtimeProvider
 * owns the 30s anchor REST cadence for chain 1 — this panel NEVER duplicates
 * it; non-default chains get ONE mount fetch and manual Refresh.
 */
"use client";

// SSR-test support (repo pattern, cf. TokenAllowlistTab/ArbxRealtimeProvider):
// the node test transformer's classic JSX path needs the React namespace in
// module scope — inert for the Next automatic-runtime app build.
import * as React from "react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { RuntimeSettingState } from "@/components/RuntimeSettingState";
import { hasAdminSession } from "@/lib/admin-token";
import { getCanonicalKnobs, previewQuoteWeights } from "@/lib/api-client";
import type {
  QuotePreviewResponse,
  QuoteWeights,
} from "@/lib/apex/schemas";
import { useOmniStore } from "@/lib/store/omni-store";

const AXES = [
  { key: "prior", label: "Prior" },
  { key: "liquidity", label: "Liquidity" },
  { key: "venues", label: "Venues" },
  { key: "stability", label: "Stability" },
  { key: "cross_dex", label: "Cross-DEX" },
] as const;

type AxisKey = (typeof AXES)[number]["key"];

/** §62-adjacent display: scores are 0..100 floats — fixed(1) is display-only. */
const fmt = (n: number | null | undefined): string =>
  n === null || n === undefined || !Number.isFinite(n) ? "—" : n.toFixed(1);

interface Props {
  chainId: number;
  adminToken: string;
  actor: string;
}

export function QuoteBasePanel({ chainId, adminToken, actor }: Props) {
  const anchor = useOmniStore((s) => s.quoteAnchor);
  const status = useOmniStore((s) => s.quoteAnchorStatus);
  const error = useOmniStore((s) => s.quoteAnchorError);
  const updatedAt = useOmniStore((s) => s.quoteAnchorUpdatedAt);
  const fetchQuoteAnchor = useOmniStore((s) => s.fetchQuoteAnchor);

  // Cadence ownership (FE-0008 / cont.42 addendum): the root
  // ArbxRealtimeProvider polls quote_anchor REST (first pass + 30s) for chain
  // 1 — this panel does NOT duplicate that loop. Non-default chains have no
  // provider coverage: ONE mount fetch back-fills; later refreshes are the
  // operator's Refresh button.
  useEffect(() => {
    if (chainId !== 1) void fetchQuoteAnchor(chainId);
  }, [chainId, fetchQuoteAnchor]);

  // §11 coherencia inputs: the canonical-knobs snapshot (env/boot config)
  // vs the runtime mirror the anchor payload carries. Read-only.
  const [knobsWeights, setKnobsWeights] = useState<QuoteWeights | null>(null);
  useEffect(() => {
    let alive = true;
    void getCanonicalKnobs().then((res) => {
      // 503 knobs_not_published / shape drift → stays null → NOT_EXPOSED (R8).
      if (alive && res.ok) setKnobsWeights(knobsToQuoteWeights(res.data.knobs));
    });
    return () => {
      alive = false;
    };
  }, []);

  // ── §10 preview state (operator input, never derived math) ───────────────
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Record<AxisKey, string>>({
    prior: "0.2", liquidity: "0.2", venues: "0.2", stability: "0.2", cross_dex: "0.2",
  });
  const [preview, setPreview] = useState<QuotePreviewResponse | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [previewMsg, setPreviewMsg] = useState<string | null>(null);
  const [hasSession, setHasSession] = useState(false);
  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  const startEditing = useCallback(() => {
    if (!anchor) return;
    // Seed the draft from the RUNTIME mirror the payload carries (§9).
    setDraft({
      prior: String(anchor.weights.prior),
      liquidity: String(anchor.weights.liquidity),
      venues: String(anchor.weights.venues),
      stability: String(anchor.weights.stability),
      cross_dex: String(anchor.weights.cross_dex),
    });
    setPreview(null);
    setPreviewMsg(null);
    setEditing(true);
  }, [anchor]);

  const draftSum = useMemo(
    () => AXES.reduce((acc, a) => acc + (Number.parseFloat(draft[a.key]) || 0), 0),
    [draft],
  );
  // Mirrors the backend knob validation tolerance exactly (R3 7b note: a
  // 1e-6 gate would let a draft in the 1e-9..1e-6 band pass FE and eat the
  // honest 400).
  const draftValid = Math.abs(draftSum - 1) <= 1e-9;

  const runPreview = async () => {
    if (!draftValid) return;
    if (!hasAdminSession()) {
      setPreviewMsg("Login required: open /killswitch and unlock an admin session first.");
      return;
    }
    setPreviewing(true);
    setPreviewMsg(null);
    const weights = Object.fromEntries(
      AXES.map((a) => [a.key, Number.parseFloat(draft[a.key])]),
    ) as QuoteWeights;
    const res = await previewQuoteWeights(chainId, weights, adminToken, actor);
    setPreviewing(false);
    if (res.ok) {
      setPreview(res.data);
    } else {
      setPreview(null);
      setPreviewMsg(res.error);
    }
  };

  return (
    <div className="space-y-4">
      {/* ── §8 Current Quote Anchor ─────────────────────────────────────── */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">Quote/Base · anchor vigente</CardTitle>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            {updatedAt && <span>updated {updatedAt}</span>}
            <Button
              variant="outline"
              size="sm"
              onClick={() => void fetchQuoteAnchor(chainId)}
              disabled={status === "loading"}
            >
              Refresh
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {status === "error" && (
            <p className="text-sm text-destructive" role="alert">
              {error ?? "quote anchor unavailable"}
            </p>
          )}
          {status !== "error" && !anchor && <p className="text-sm text-muted-foreground">—</p>}
          {anchor && (
            <div className="space-y-4">
              <div className="flex flex-wrap items-baseline gap-x-6 gap-y-2">
                <div>
                  <span className="text-3xl font-semibold tracking-tight">{anchor.quote_symbol}</span>
                  <span className="ml-2 text-sm text-muted-foreground">score {fmt(anchor.quote_score)}/100</span>
                </div>
                <Badge variant="outline">quote_version {anchor.quote_version}</Badge>
                <Badge variant="outline">graph_version {anchor.graph_version}</Badge>
              </div>
              <div className="grid gap-4 md:grid-cols-2">
                <div>
                  <p className="mb-2 text-xs font-medium uppercase text-muted-foreground">
                    Componentes del score (§9)
                  </p>
                  <div className="space-y-1.5">
                    {AXES.map((a) => (
                      <div key={a.key} className="flex items-center gap-2">
                        <span className="w-20 text-xs">{a.label}</span>
                        <div className="h-2 flex-1 overflow-hidden rounded bg-muted">
                          <div
                            className="h-full bg-primary"
                            style={{ width: `${Math.min(100, Math.max(0, anchor.components[a.key]))}%` }}
                          />
                        </div>
                        <span className="w-10 text-right text-xs tabular-nums">
                          {fmt(anchor.components[a.key])}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
                <div>
                  <p className="mb-2 text-xs font-medium uppercase text-muted-foreground">
                    Pesos runtime (espejo de quote_w_*)
                  </p>
                  <div className="space-y-1.5">
                    {AXES.map((a) => (
                      <div key={a.key} className="flex items-center justify-between text-xs">
                        <span>{a.label}</span>
                        <span className="tabular-nums">{anchor.weights[a.key].toFixed(3)}</span>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          )}
          {/* ── §11 versionado: coherencia knobs ↔ runtime ───────────────── */}
          <QuoteWeightsCoherency snapshot={knobsWeights} mirror={anchor?.weights ?? null} />
        </CardContent>
      </Card>

      {/* ── §9 explainable per-token table ──────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Score explicable por token (§9)</CardTitle>
        </CardHeader>
        <CardContent>
          {anchor && anchor.tokens.length > 0 && (
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <th className="py-1.5 pr-3 font-medium">Token</th>
                    <th className="py-1.5 pr-3 font-medium">Address</th>
                    {AXES.map((a) => (
                      <th key={a.key} className="py-1.5 pr-3 text-right font-medium">{a.label}</th>
                    ))}
                    <th className="py-1.5 text-right font-medium">Score</th>
                  </tr>
                </thead>
                <tbody>
                  {anchor.tokens.map((t) => (
                    <tr key={`${t.symbol}-${t.address}`} className="border-b last:border-0">
                      <td className="py-1.5 pr-3 font-medium">{t.symbol}</td>
                      <td className="py-1.5 pr-3 font-mono text-muted-foreground">
                        {t.address.slice(0, 8)}…{t.address.slice(-6)}
                      </td>
                      {AXES.map((a) => (
                        <td key={a.key} className="py-1.5 pr-3 text-right tabular-nums">
                          {fmt(t.components[a.key])}
                        </td>
                      ))}
                      <td className="py-1.5 text-right tabular-nums font-medium">{fmt(t.score)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {anchor && anchor.tokens.length === 0 && (
            <p className="text-sm text-muted-foreground">
              Sin tokens computables este tick (candidatura exige símbolo + precio + pool valorado).
            </p>
          )}
          {!anchor && <p className="text-sm text-muted-foreground">—</p>}
        </CardContent>
      </Card>

      {/* ── §10 preview before apply (admin, no mutation) ───────────────── */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">Preview de pesos (§10 · sin mutación)</CardTitle>
          {!editing && (
            <Button variant="outline" size="sm" onClick={startEditing} disabled={!anchor}>
              Proponer pesos
            </Button>
          )}
        </CardHeader>
        <CardContent className="space-y-4">
          {!editing && (
            <p className="text-sm text-muted-foreground">
              Re-rankeo determinístico de las MISMAS filas bajo pesos propuestos — el grafo nunca se
              reconstruye (QB-TOPOLOGY-01). El apply fluye por los knobs canónicos, no por aquí.
            </p>
          )}
          {editing && (
            <>
              <div className="grid gap-3 sm:grid-cols-5">
                {AXES.map((a) => (
                  <div key={a.key} className="space-y-1">
                    <label htmlFor={`qw-${a.key}`} className="text-xs font-medium">
                      {a.label}
                    </label>
                    <Input
                      id={`qw-${a.key}`}
                      type="number"
                      min={0}
                      max={1}
                      step={0.05}
                      value={draft[a.key]}
                      onChange={(e) => setDraft((d) => ({ ...d, [a.key]: e.target.value }))}
                    />
                  </div>
                ))}
              </div>
              <div className="flex items-center gap-3">
                <Button size="sm" onClick={() => void runPreview()} disabled={!draftValid || previewing}>
                  {previewing ? "Preview…" : "Preview"}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setEditing(false);
                    setPreview(null);
                    setPreviewMsg(null);
                  }}
                >
                  Cancelar
                </Button>
                <span className={`text-xs tabular-nums ${draftValid ? "text-muted-foreground" : "text-destructive"}`}>
                  Σ = {draftSum.toFixed(4)} {draftValid ? "" : "(debe ser 1 ± 1e-9)"}
                </span>
              </div>
            </>
          )}
          {previewMsg && <p className="text-sm text-destructive" role="alert">{previewMsg}</p>}
          {preview && <PreviewResult preview={preview} />}
        </CardContent>
      </Card>
    </div>
  );
}

// ─── FE-0015 · §10 preview result — pure, exported for direct testing ──────

/**
 * The deterministic backend re-ranking the §10 POST returned: impact badges
 * (QB-TOPOLOGY-01 literals included) + the proposed table in payload order
 * with the proposed-anchor row highlighted. Props in, markup out — §79.
 */
export function PreviewResult({ preview }: { preview: QuotePreviewResponse }) {
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant={preview.impact.quote_revaluation_required ? "default" : "secondary"}>
          {preview.impact.quote_revaluation_required
            ? `Cambia anchor → ${preview.proposed_quote_symbol}`
            : "Anchor sin cambio"}
        </Badge>
        <Badge variant="outline">quote_version {preview.impact.current_quote_version} → {preview.impact.proposed_quote_version}</Badge>
        <Badge variant="outline">pares afectados {preview.impact.affected_pairs}</Badge>
        <Badge variant="outline">edges afectados {preview.impact.affected_edges}</Badge>
        <Badge variant="outline">rutas cacheadas {preview.impact.affected_cached_routes}</Badge>
        <span className="text-xs text-muted-foreground">
          graph_rebuild_required {String(preview.impact.graph_rebuild_required)} · topology_unchanged {String(preview.impact.topology_version_unchanged)}
        </span>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b text-left text-muted-foreground">
              <th className="py-1.5 pr-3 font-medium">Token</th>
              <th className="py-1.5 pr-3 text-right font-medium">Prior</th>
              <th className="py-1.5 pr-3 text-right font-medium">Liquidity</th>
              <th className="py-1.5 pr-3 text-right font-medium">Venues</th>
              <th className="py-1.5 pr-3 text-right font-medium">Stability</th>
              <th className="py-1.5 pr-3 text-right font-medium">Cross-DEX</th>
              <th className="py-1.5 text-right font-medium">Score propuesto</th>
            </tr>
          </thead>
          <tbody>
            {preview.proposed_tokens.map((t) => (
              <tr
                key={`${t.symbol}-${t.address}`}
                className={`border-b last:border-0 ${t.symbol === preview.proposed_quote_symbol ? "bg-muted/50" : ""}`}
              >
                <td className="py-1.5 pr-3 font-medium">{t.symbol}</td>
                <td className="py-1.5 pr-3 text-right tabular-nums">{fmt(t.components.prior)}</td>
                <td className="py-1.5 pr-3 text-right tabular-nums">{fmt(t.components.liquidity)}</td>
                <td className="py-1.5 pr-3 text-right tabular-nums">{fmt(t.components.venues)}</td>
                <td className="py-1.5 pr-3 text-right tabular-nums">{fmt(t.components.stability)}</td>
                <td className="py-1.5 pr-3 text-right tabular-nums">{fmt(t.components.cross_dex)}</td>
                <td className="py-1.5 text-right tabular-nums font-medium">{fmt(t.score)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// ─── FE-0016 · §11 versionado — coherencia knobs ↔ runtime ────────────────

/** Canonical fixed(6) serialization — the coherencia comparison key. */
export function quoteWeightsKey(w: QuoteWeights): string {
  return [w.prior, w.liquidity, w.venues, w.stability, w.cross_dex]
    .map((x) => x.toFixed(6))
    .join("/");
}

/**
 * Maps the canonical-knobs snapshot record (serde snake_case) onto the mirror
 * shape. A missing/non-finite knob → null (NOT_EXPOSED — never zero-filled,
 * R8): the snapshot's key set is the searcher's, not this panel's to assume.
 */
export function knobsToQuoteWeights(knobs: Record<string, unknown>): QuoteWeights | null {
  const g = (k: string): number | null => {
    const v = knobs[k];
    return typeof v === "number" && Number.isFinite(v) ? v : null;
  };
  const prior = g("quote_w_prior");
  const liquidity = g("quote_w_liquidity");
  const venues = g("quote_w_venue_coverage");
  const stability = g("quote_w_stability");
  const cross_dex = g("quote_w_cross_dex");
  if (prior === null || liquidity === null || venues === null || stability === null || cross_dex === null) {
    return null;
  }
  return { prior, liquidity, venues, stability, cross_dex };
}

/**
 * §11 coherencia line over FE-0005's RuntimeSettingState: configured = the
 * knobs snapshot (env/boot), effective = the runtime mirror the live anchor
 * carries. ackEventId is ALWAYS null here — quote weights have no runtime
 * mutation path (the §10 preview never applies; §3: nothing pretends a 200
 * was an ACK). Steady states only: EFFECTIVE / CONFIGURED / NOT_EXPOSED.
 */
export function QuoteWeightsCoherency({
  snapshot,
  mirror,
}: {
  snapshot: QuoteWeights | null;
  mirror: QuoteWeights | null;
}) {
  if (snapshot === null) {
    return (
      <p className="text-xs text-muted-foreground">
        §11 coherencia: knobs snapshot no servido — quote_w_* configurados no
        computados (R8), nunca cero.
      </p>
    );
  }
  return (
    <RuntimeSettingState
      label="quote_w_* knobs ↔ runtime"
      configured={quoteWeightsKey(snapshot)}
      effective={mirror === null ? null : quoteWeightsKey(mirror)}
      ackEventId={null}
    />
  );
}
