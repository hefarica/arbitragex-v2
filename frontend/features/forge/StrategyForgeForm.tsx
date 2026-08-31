"use client";

/**
 * StrategyForgeForm — Idea 2: inject + manage dynamic strategy cartridges.
 *
 * Operates over the EXISTING admin-gated cartridge-forge endpoints (POST /api/v1/cartridges
 * to inject, pause/resume/delete for lifecycle). Injecting publishes arbx:cartridge:injection
 * → searcher-rs CartridgeSubscriber compiles + hot-loads the Rhai into the SHADOW cartridge
 * runtime (admin-gated, sandboxed, NO capital / no execution). The banner says so.
 *
 * R8 fail-honest: lists exactly what the registry returns; surfaces upstream errors verbatim;
 * "—" for unknown. Deterministic initial render (no Date/random) so it hydrates cleanly.
 */

import * as React from "react";
import { HammerIcon, ShieldCheckIcon, AlertCircleIcon, PlayIcon, PauseIcon, Trash2Icon, RocketIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import { Dropzone, type UploadedScript } from "@/components/ui/dropzone";
import { useCartridgeForge, type MutationResult } from "./useCartridgeForge";
import { SLUG_RE } from "./cartridge-forge-types";

function decodeBase64(b64: string): string {
  try {
    return typeof window !== "undefined" ? window.atob(b64) : "";
  } catch {
    return "";
  }
}

function parseChains(raw: string): number[] {
  return raw
    .split(",")
    .map((s) => Number(s.trim()))
    .filter((n) => Number.isInteger(n) && n > 0);
}

function StateBadge({ state }: { state: string }) {
  const s = state.toLowerCase();
  const variant = s === "active" ? "default" : s === "paused" ? "secondary" : "outline";
  return (
    <Badge variant={variant} className="text-[10px] uppercase tracking-wide">
      {state}
    </Badge>
  );
}

export function StrategyForgeForm() {
  const { cartridges, status, error, inject, pause, resume, remove } = useCartridgeForge();

  const [slug, setSlug] = React.useState("");
  const [source, setSource] = React.useState("");
  const [chainsRaw, setChainsRaw] = React.useState("");
  const [uploaded, setUploaded] = React.useState<UploadedScript | null>(null);
  const [result, setResult] = React.useState<MutationResult | null>(null);
  const [busy, setBusy] = React.useState(false);

  const slugValid = SLUG_RE.test(slug);
  const canInject = slugValid && source.trim().length > 0 && !busy;

  const onUpload = (file: UploadedScript | null) => {
    setUploaded(file);
    if (file) setSource(decodeBase64(file.content));
  };

  const onInject = async () => {
    setBusy(true);
    setResult(null);
    const r = await inject({ slug, source_code: source, target_chains: parseChains(chainsRaw) }, "operator");
    setResult(r);
    setBusy(false);
    if (r.ok) {
      setSlug("");
      setSource("");
      setChainsRaw("");
      setUploaded(null);
    }
  };

  const onLifecycle = async (action: (slug: string, actor: string) => Promise<MutationResult>, s: string) => {
    setBusy(true);
    setResult(null);
    const r = await action(s, "operator");
    setResult(r);
    setBusy(false);
  };

  return (
    <Card data-slot="strategy-forge-form">
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <HammerIcon className="h-5 w-5 text-primary" />
            Strategy Forge — Cartridge Injection
          </CardTitle>
          <Badge variant="outline" className="gap-1 text-[10px] text-muted-foreground">
            <ShieldCheckIcon className="h-3 w-3" />
            admin · shadow eval
          </Badge>
        </div>
        <Alert>
          <RocketIcon className="h-4 w-4" />
          <AlertTitle>Injects into the shadow cartridge runtime</AlertTitle>
          <AlertDescription className="text-xs">
            Injecting hot-loads the Rhai into searcher-rs and evaluates it in <span className="font-semibold">shadow</span> —
            sandboxed, admin-gated, <span className="font-semibold">no capital / no execution</span>. Opportunities a
            cartridge finds flow through the same paper pipeline. Requires an admin session.
          </AlertDescription>
        </Alert>
      </CardHeader>

      <CardContent className="space-y-5">
        {/* ── Inject form ─────────────────────────────────────────── */}
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="forge-slug">Slug</Label>
            <Input
              id="forge-slug"
              value={slug}
              onChange={(e) => setSlug(e.target.value)}
              placeholder="my_cartridge_v1"
              className={cn("font-mono", slug.length > 0 && !slugValid && "border-destructive")}
            />
            <p className="text-[11px] text-muted-foreground">
              {slug.length > 0 && !slugValid
                ? "3–49 chars, lowercase a-z0-9_, must start with a letter"
                : "unique id; lowercase a-z0-9_"}
            </p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="forge-chains">Target chains</Label>
            <Input
              id="forge-chains"
              value={chainsRaw}
              onChange={(e) => setChainsRaw(e.target.value)}
              placeholder="1, 42161"
              className="font-mono"
            />
            <p className="text-[11px] text-muted-foreground">comma-separated chain IDs · empty = all chains</p>
          </div>
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="forge-source">Cartridge source (.rhai)</Label>
          <Dropzone accept={["rhai"]} value={uploaded} onChange={onUpload} />
          <textarea
            id="forge-source"
            value={source}
            onChange={(e) => setSource(e.target.value)}
            spellCheck={false}
            rows={8}
            placeholder="// upload a .rhai above, or paste/edit the cartridge source here&#10;fn evaluate(state) { /* ... */ }"
            className="w-full rounded-md border bg-muted/30 p-3 font-mono text-xs leading-relaxed outline-none focus-visible:ring-1 focus-visible:ring-ring"
          />
        </div>

        {result ? (
          <Alert variant={result.ok ? "default" : "destructive"}>
            <AlertCircleIcon className="h-4 w-4" />
            <AlertTitle>{result.ok ? "Done" : "Action failed"}</AlertTitle>
            <AlertDescription className="text-xs">
              {result.ok ? (result.message ?? "Cartridge injected.") : (
                <>
                  <span className="font-mono">{result.error}</span>
                  {result.error === "missing_admin_token" || result.error?.startsWith("http_401")
                    ? " — admin session required (you are not signed in as operator)."
                    : null}
                </>
              )}
            </AlertDescription>
          </Alert>
        ) : null}

        <div className="flex justify-end">
          <Button onClick={onInject} disabled={!canInject} className="gap-1.5">
            <RocketIcon className="h-4 w-4" />
            {busy ? "Working…" : "Inject cartridge"}
          </Button>
        </div>

        {/* ── Registry ────────────────────────────────────────────── */}
        <Separator />
        <div className="flex items-center justify-between">
          <h4 className="text-sm font-semibold">Injected cartridges</h4>
          <span className="text-xs text-muted-foreground">
            {status === "loading" ? "loading…" : `${cartridges.length} registered`}
          </span>
        </div>

        {error && cartridges.length === 0 ? (
          <Alert variant="destructive">
            <AlertCircleIcon className="h-4 w-4" />
            <AlertTitle>Registry unavailable</AlertTitle>
            <AlertDescription className="text-xs font-mono">{error}</AlertDescription>
          </Alert>
        ) : cartridges.length === 0 && status !== "loading" ? (
          <p className="rounded-md border bg-muted/20 p-4 text-center text-sm text-muted-foreground">
            No cartridges injected yet.
          </p>
        ) : (
          <div className="space-y-2">
            {cartridges.map((c) => {
              const active = c.state.toLowerCase() === "active";
              const paused = c.state.toLowerCase() === "paused";
              return (
                <div key={c.id || c.slug} className="rounded-lg border p-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-sm font-medium">{c.slug}</span>
                      <StateBadge state={c.state} />
                      <span className="text-[11px] text-muted-foreground">
                        v{c.version} · {c.target_chains.length === 0 ? "all chains" : c.target_chains.join(",")}
                      </span>
                    </div>
                    <div className="flex items-center gap-1">
                      {active ? (
                        <Button size="sm" variant="outline" className="h-7 gap-1 px-2 text-[11px]" disabled={busy}
                          onClick={() => void onLifecycle(pause, c.slug)}>
                          <PauseIcon className="h-3 w-3" /> Pause
                        </Button>
                      ) : null}
                      {paused ? (
                        <Button size="sm" variant="outline" className="h-7 gap-1 px-2 text-[11px]" disabled={busy}
                          onClick={() => void onLifecycle(resume, c.slug)}>
                          <PlayIcon className="h-3 w-3" /> Resume
                        </Button>
                      ) : null}
                      <Button size="sm" variant="outline" className="h-7 gap-1 px-2 text-[11px] text-destructive border-destructive/40" disabled={busy}
                        onClick={() => void onLifecycle(remove, c.slug)}>
                        <Trash2Icon className="h-3 w-3" /> Delete
                      </Button>
                    </div>
                  </div>
                  <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 font-mono text-[11px] text-muted-foreground tabular-nums">
                    <span>evals {c.total_evaluations.toLocaleString()}</span>
                    <span className={cn(c.total_opportunities > 0 && "text-success")}>opps {c.total_opportunities.toLocaleString()}</span>
                    <span className={cn(c.total_errors > 0 && "text-warning")}>errors {c.total_errors.toLocaleString()}</span>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
