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

import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
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

  const add = () => {
    const v = pending.trim().toUpperCase();
    if (!v || v.length > 16) return;
    setSymbols((prev) => Array.from(new Set([...prev, v])));
    setPending("");
  };

  const remove = (sym: string) => setSymbols((prev) => prev.filter((s) => s !== sym));

  const onSave = async () => {
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
            placeholder="add symbol (e.g. WETH)"
            onChange={(e) => setPending(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                add();
              }
            }}
            className="max-w-xs"
          />
          <Button variant="outline" onClick={add}>Add</Button>
          <Button onClick={onSave} disabled={saving} className="ml-auto">
            {saving ? "Saving…" : "Save changes"}
          </Button>
        </div>
        {msg && <p className="text-xs text-muted-foreground font-mono">{msg}</p>}
      </CardContent>
    </Card>
  );
}
