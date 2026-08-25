/**
 * FE-MASTER · Token allowlist tab — P3 evolution (FE-0010/0011/0012, §4-§6).
 *
 * PRESERVE (Sprint 2.3 UX): quick symbol input + chips + remove + the
 * compat save over `trading_config.allowed_token_symbols` (still the wire
 * field — the canonical TokenKey set lives backend-side, EMIT-01/04).
 *
 * EXTEND (§5 flow): Resolve button → POST /api/admin/tokens/resolve (EMIT-01)
 * → preview table, one row per REQUESTED symbol with its honest
 * resolution_status. FE-0012: only RESOLVED symbols may be saved as active —
 * AMBIGUOUS/NOT_FOUND/UNSUPPORTED block the save with the reason listed, and
 * symbols added after the last preview (uncovered) block too (the UI shows,
 * the resolver decides — never the reverse).
 *
 * FE-0011: universe consequence KPIs (§6) render from the UniverseSlice the
 * resolve response records (N, C(N,2), N(N−1), E, V, versions — all
 * backend-derived, §79: never computed in React; null = "—").
 *
 * FE-0016 (§11/§3): the save feedback is the FE-0005 RuntimeSettingState line
 * wired to the PUT's universe_version + runtime_ack_event_id (EMIT-04 wire) —
 * HTTP 200 is NOT "applied"; only the runtime ACK broadcast (event_id
 * bijection) moves the line past WAITING, and the refetched universe_version
 * decides VERIFIED vs DRIFT. The "Saved · scanner sees…" lie is gone.
 */
"use client";

// SSR-test support (repo pattern, cf. TokenIcon/ChainsAdminClient): the node
// test env renders the pure exports of this module via react-dom/server with
// jsx preserved, so React must be in module scope.
import * as React from "react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { RuntimeSettingState } from "@/components/RuntimeSettingState";
import { hasAdminSession } from "@/lib/admin-token";
import { putTradingConfig, resolveTokens } from "@/lib/api-client";
import { shortAddr } from "@/lib/format";
import { useOmniStore } from "@/lib/store/omni-store";
import type { TradingConfigConfigured } from "@/lib/schemas";
import type { TokenResolvePreviewRow, TokenUniverseKpi } from "@/lib/apex/schemas";

const DASH = "—";

/** Badge variant per resolution status — color is never the only signal. */
const STATUS_VARIANT: Record<TokenResolvePreviewRow["resolution_status"], "secondary" | "outline" | "destructive"> = {
  RESOLVED: "secondary",
  AMBIGUOUS: "outline",
  NOT_FOUND: "destructive",
  UNSUPPORTED: "outline",
};

interface Props {
  config: TradingConfigConfigured;
  onSaved: (next: TradingConfigConfigured) => void;
  adminToken: string;
  actor: string;
}

export function TokenAllowlistTab({ config, onSaved, adminToken, actor }: Props) {
  const [symbols, setSymbols] = useState<string[]>(config.allowed_token_symbols.map((s) => s.toUpperCase()));
  const [pending, setPending] = useState("");
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  // §5 preview — one row per requested symbol; null = never resolved.
  const [preview, setPreview] = useState<TokenResolvePreviewRow[] | null>(null);
  const [resolving, setResolving] = useState(false);
  const [resolveError, setResolveError] = useState<string | null>(null);
  // R1: session check deferred to useEffect — never read document.cookie during SSR.
  const [hasSession, setHasSession] = useState(false);
  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  // FE-0011: KPIs from the UniverseSlice (recorded by the resolve response).
  const universe = useOmniStore((s) => s.universe);
  const setResolveResult = useOmniStore((s) => s.setResolveResult);

  // FE-0016: what the last save stamped — universe_version (null = the edit
  // left the universe untouched: no bump, no ACK row) + the ack event_id.
  const [saveAck, setSaveAck] = useState<{ eventId: string | null; version: number | null }>({
    eventId: null,
    version: null,
  });

  const add = () => {
    // Accept multi-symbol input: comma, semicolon, newline, or whitespace separated.
    // "WETH, USDC, USDT" → 3 entries; "WETH" → 1 entry. Empty + over-16-char dropped.
    const tokens = pending
      .split(/[,;\s\n\r\t]+/)
      .map((s) => s.trim().toUpperCase())
      .filter((s) => s.length > 0 && s.length <= 16);
    if (tokens.length === 0) return;
    setSymbols((prev) => Array.from(new Set([...prev, ...tokens])));
    setPending("");
  };

  const remove = (sym: string) => setSymbols((prev) => prev.filter((s) => s !== sym));

  // §5: parse (client input) → dedupe (Set) → resolve per chain (backend) →
  // preview. The response's universe block lands in the UniverseSlice (KPIs).
  const onResolve = async () => {
    setResolving(true);
    setResolveError(null);
    const res = await resolveTokens(config.chain_id, symbols, adminToken, actor);
    setResolving(false);
    if (res.ok) {
      setPreview(res.data.results);
      setResolveResult(res.data);
      setMsg(null);
    } else {
      // Honest error verbatim — never a fabricated preview (RULE 00).
      setResolveError(res.error);
    }
  };

  // FE-0012 save gate: every draft symbol must be covered by a RESOLVED row
  // of the CURRENT preview. Unresolved symbols are listed, not dropped
  // silently — the operator removes them or fixes the input.
  const statusBy = new Map(
    (preview ?? []).map((r) => [r.input_symbol.toUpperCase(), r.resolution_status]),
  );
  const blocked = unresolvedSymbols(symbols, preview);

  const onSave = async () => {
    if (!hasAdminSession()) {
      setHasSession(false);  // sync React state with reality
      setMsg("Login required: open /killswitch and unlock an admin session first.");
      return;
    }
    if (preview === null) {
      setMsg("Resolve primero — el save exige tokens validados (§5: parse→dedupe→resolve→preview→validar→save).");
      return;
    }
    if (blocked.length > 0) {
      setMsg(
        `Bloqueado (§5): ${blocked.map((s) => `${s}=${statusBy.get(s)}`).join(", ")} — solo tokens RESOLVED pueden guardarse como activos.`,
      );
      return;
    }
    setSaving(true);
    setMsg(null);
    const { configured: _c, chain_id: _cid, updated_at: _ua, updated_by: _ub, ...rest } = config;
    void _c; void _cid; void _ua; void _ub;
    const body = { ...rest, allowed_token_symbols: symbols };
    const res = await putTradingConfig(config.chain_id, body, adminToken, actor);
    setSaving(false);
    if (res.ok) {
      onSaved({ ...config, allowed_token_symbols: symbols, updated_at: res.data.updated_at, updated_by: res.data.updated_by });
      setSaveAck({ eventId: res.data.runtime_ack_event_id, version: res.data.universe_version });
      if (res.data.universe_version === null) {
        // Honest: a knobs-only edit bumps nothing — there is no versioned ACK
        // wire for it (EMIT-04: only an allowed_token_symbols change bumps).
        setMsg("Saved · universo sin cambio — sin ACK versionado.");
      } else {
        setMsg(null);
      }
    } else {
      setMsg(res.error);
    }
  };

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>Token allowlist · base = {config.base_token_symbol}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap gap-2">
            {symbols.length === 0 && (
              <span className="text-xs text-muted-foreground">No tokens allowed — chain is idle.</span>
            )}
            {symbols.map((s) => {
              const st = statusBy.get(s);
              return (
                <Badge
                  key={s}
                  variant={st === undefined ? "outline" : STATUS_VARIANT[st]}
                  className="cursor-pointer"
                  title={st === undefined ? "sin resolver todavía (§5)" : st}
                  onClick={() => remove(s)}
                >
                  {s} ×
                </Badge>
              );
            })}
          </div>
          <div className="flex items-center gap-2">
            <Input
              value={pending}
              placeholder="add one or more symbols — paste comma-separated: WETH, USDC, USDT"
              onChange={(e) => setPending(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  add();
                }
              }}
              className="flex-1"
            />
            <Button variant="outline" onClick={add}>Add</Button>
            <Button
              variant="outline"
              onClick={() => void onResolve()}
              disabled={resolving || symbols.length === 0}
              title="§5: resolve per chain contra el TokenIdentityIndex del backend"
            >
              {resolving ? "Resolving…" : "Resolve"}
            </Button>
            <Button onClick={onSave} disabled={saving || !hasSession} className="ml-auto" title={!hasSession ? "Admin session required — open /killswitch first" : undefined}>
              {saving ? "Saving…" : "Save changes"}
            </Button>
          </div>
          {msg && <p className="text-xs text-muted-foreground font-mono">{msg}</p>}
          {saveAck.version !== null && (
            <UniverseSaveCoherency
              putVersion={saveAck.version}
              ackEventId={saveAck.eventId}
              symbolsCount={symbols.length}
              universe={universe}
              onAckApplied={() => void onResolve()}
            />
          )}
          {resolveError && (
            <p className="text-xs text-destructive font-mono" role="alert">{resolveError}</p>
          )}
          {!hasSession && !msg && (
            <p className="text-xs text-warning font-mono">No admin session — <a href="/killswitch" className="underline">unlock at /killswitch</a></p>
          )}
        </CardContent>
      </Card>
      <UniverseKpiCards universe={universe} />
      {preview !== null && (
        <TokenResolvePreviewTable rows={preview} chainId={config.chain_id} />
      )}
    </div>
  );
}

// ─── FE-0012 · Save gate (§5) — pure, exported for direct testing ─────────

/**
 * Draft symbols that may NOT be saved as active: those without a RESOLVED row
 * in the current preview (AMBIGUOUS / NOT_FOUND / UNSUPPORTED) AND those the
 * preview does not cover at all (added after the last resolve, or never
 * resolved — preview null blocks everything). Pure: the UI shows the list,
 * the resolver's statuses decide — never the reverse.
 */
export function unresolvedSymbols(
  symbols: string[],
  preview: TokenResolvePreviewRow[] | null,
): string[] {
  const statusBy = new Map(
    (preview ?? []).map((r) => [r.input_symbol.toUpperCase(), r.resolution_status]),
  );
  return symbols.filter((s) => statusBy.get(s) !== "RESOLVED");
}

// ─── FE-0011 · Universe consequence KPIs (§6) — pure, props in ────────────

const KPI_DEFS: { key: keyof TokenUniverseKpi; label: string; title: string }[] = [
  { key: "allowed_tokens", label: "Tokens N", title: "allowlist efectiva (set canónico TokenKey)" },
  { key: "possible_pairs", label: "Pares C(N,2)", title: "Σ_c C(N_c,2) — pares posibles por chain" },
  { key: "directed_token_pairs", label: "Dirigidos N(N−1)", title: "Σ_c N_c(N_c−1) — edges dirigidos" },
  { key: "active_pools", label: "Pools E", title: "pools activos del grafo vivo" },
  { key: "active_venues", label: "Venues V", title: "venues con ≥1 pool activo" },
  { key: "graph_version", label: "Graph version", title: "versión del grafo (graph_builder)" },
  { key: "universe_version", label: "Universe version", title: "bump al cambiar la allowlist (EMIT-04)" },
];

export function UniverseKpiCards({ universe }: { universe: TokenUniverseKpi | null }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">
          Universo · consecuencia (§6)
          <span className="ml-2 text-sm font-normal text-muted-foreground">
            combinatorias derivadas del backend — jamás en React (§79)
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-7">
          {KPI_DEFS.map(({ key, label, title }) => {
            const v = universe?.[key] ?? null;
            return (
              <div key={key} className="rounded-md border border-border/60 p-2" title={title}>
                <div className="text-[10px] text-muted-foreground">{label}</div>
                <div className="text-base font-medium tabular-nums">{v === null ? DASH : v}</div>
              </div>
            );
          })}
        </div>
        {universe === null && (
          <p className="mt-2 text-xs text-muted-foreground">
            Sin KPIs servidos aún — resuelve el set para poblarlos (R8: null = no servido, no cero).
          </p>
        )}
      </CardContent>
    </Card>
  );
}

// ─── FE-0010/0012 · Quick-resolve preview table (§5) — pure, props in ─────

export function TokenResolvePreviewTable({ rows, chainId }: { rows: TokenResolvePreviewRow[]; chainId: number }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">
          Preview resolución (§5)
          <span className="ml-2 text-sm font-normal text-muted-foreground">
            chain {chainId} · {rows.length} símbolos pedidos
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow className="text-left text-muted-foreground">
                <TableHead className="font-medium">Símbolo</TableHead>
                <TableHead className="font-medium">Estado</TableHead>
                <TableHead className="font-medium">Address</TableHead>
                <TableHead className="text-right font-medium">Decimals</TableHead>
                <TableHead className="text-right font-medium">Pools</TableHead>
                <TableHead className="text-right font-medium">Venues</TableHead>
                <TableHead className="text-right font-medium">Liquidity USD</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((r) => (
                <TableRow key={`${r.chain_id}:${r.input_symbol}`}>
                  <TableCell className="font-medium">{r.input_symbol}</TableCell>
                  <TableCell>
                    <Badge variant={STATUS_VARIANT[r.resolution_status]}>{r.resolution_status}</Badge>
                  </TableCell>
                  <TableCell className="font-mono text-xs" title={r.address ?? undefined}>
                    {r.address === null ? DASH : shortAddr(r.address)}
                  </TableCell>
                  <TableCell className="text-right tabular-nums">{r.decimals === null ? DASH : r.decimals}</TableCell>
                  <TableCell className="text-right tabular-nums">{r.pool_count === null ? DASH : r.pool_count}</TableCell>
                  <TableCell className="text-right tabular-nums">{r.venue_count === null ? DASH : r.venue_count}</TableCell>
                  {/* §62: liquidity rides as a decimal string, verbatim. */}
                  <TableCell className="text-right tabular-nums">{r.liquidity_usd === null ? DASH : r.liquidity_usd}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
        <p className="mt-2 text-xs text-muted-foreground">
          Estados honestos (§4/§5): solo RESOLVED puede guardarse como activo —
          AMBIGUOUS · NOT_FOUND · UNSUPPORTED bloquean el save con la razón.
          Address/decimals/pools null = no computado, nunca un guess (R8).
        </p>
      </CardContent>
    </Card>
  );
}

// ─── FE-0016 · §11 save coherencia — pure wrapper over FE-0005 ────────────

/**
 * The post-save line: configured = the persisted allowlist (count the operator
 * PUT), effective = the universe KPIs the resolve endpoint serves live;
 * version = the PUT-stamped universe_version vs the served one. The ACK
 * broadcast (event_id bijection) moves it past WAITING; onAckApplied refetches
 * the KPIs so VERIFIED/DRIFT is decided against served truth, never assumed.
 */
export function UniverseSaveCoherency({
  putVersion,
  ackEventId,
  symbolsCount,
  universe,
  onAckApplied,
}: {
  putVersion: number;
  ackEventId: string | null;
  symbolsCount: number;
  universe: TokenUniverseKpi | null;
  onAckApplied?: () => void;
}) {
  return (
    <RuntimeSettingState
      label="token universe"
      configured={symbolsCount}
      effective={universe?.allowed_tokens ?? null}
      version={{ configured: putVersion, effective: universe?.universe_version ?? null }}
      ackEventId={ackEventId}
      onAckApplied={onAckApplied}
    />
  );
}
