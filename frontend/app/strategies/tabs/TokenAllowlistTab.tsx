/**
 * Sprint 2 Task 2.3 — Token allowlist tab.
 *
 * Edits `trading_config.allowed_token_symbols TEXT[]`. Symbols are
 * upper-cased and deduplicated client-side; backend re-validates length
 * (1..16 chars). The searcher's `token_allowed()` helper does
 * case-insensitive comparison so casing doesn't matter for runtime —
 * only for display.
 */
"use client";

import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { hasAdminSession } from "@/lib/admin-token";
import { putTradingConfig } from "@/lib/api-client";
import type { TradingConfigConfigured } from "@/lib/schemas";

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
  // R1: session check deferred to useEffect — never read document.cookie during SSR.
  const [hasSession, setHasSession] = useState(false);
  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

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

  const onSave = async () => {
    if (!hasSession) {
      setMsg("Login required: open /killswitch and unlock an admin session first.");
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
      setMsg("Saved · scanner sees new allowlist in ≤1s");
    } else {
      setMsg(res.error);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Token allowlist · base = {config.base_token_symbol}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap gap-2">
          {symbols.length === 0 && (
            <span className="text-xs text-muted-foreground">No tokens allowed — chain is idle.</span>
          )}
          {symbols.map((s) => (
            <Badge key={s} variant="secondary" className="cursor-pointer" onClick={() => remove(s)}>
              {s} ×
            </Badge>
          ))}
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
          <Button onClick={onSave} disabled={saving || !hasSession} className="ml-auto" title={!hasSession ? "Admin session required — open /killswitch first" : undefined}>
            {saving ? "Saving…" : "Save changes"}
          </Button>
        </div>
        {msg && <p className="text-xs text-muted-foreground font-mono">{msg}</p>}
        {!hasSession && !msg && (
          <p className="text-xs text-amber-400 font-mono">No admin session — <a href="/killswitch" className="underline">unlock at /killswitch</a></p>
        )}
      </CardContent>
    </Card>
  );
}
