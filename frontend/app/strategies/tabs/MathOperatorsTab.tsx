/**
 * Math Operators tab — the 31 topological mathematical concepts served by the
 * math-engine. Each is a toggle (enable/disable) applied to the math-engine
 * runtime registry.
 *
 * Per-concept detail shows which of the 264 strategies it applies to (from the
 * 264×31 strategy_mapping matrix) so the operator can see the blast radius of
 * a toggle before flipping it.
 *
 * Data: GET /api/math/operators (math-engine service via api-server proxy).
 * Toggle: POST /api/math/operators/:id/toggle (admin-gated). R8 fail-honest:
 * math-engine unreachable → explicit unavailable state, never fabricated ops.
 */
"use client";

import { useCallback, useEffect, useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { getMathOperators, toggleMathOperator } from "@/lib/api-client";
import { hasAdminSession } from "@/lib/admin-token";
import type { MathOperator } from "@/lib/schemas";
import { STRATEGY_MAPPING, operatorStrategyCount } from "@/lib/math-operator-mapping";

const POLL_MS = 5000;

interface Props {
  adminToken: string;
  actor: string;
}

export function MathOperatorsTab({ adminToken, actor }: Props) {
  const [operators, setOperators] = useState<MathOperator[]>([]);
  const [available, setAvailable] = useState<boolean | null>(null);
  const [reason, setReason] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [toggling, setToggling] = useState<number | null>(null);
  const [hasSession, setHasSession] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  const load = useCallback(async () => {
    const r = await getMathOperators();
    if (r.ok) {
      setOperators(r.data);
      setAvailable(true);
      setReason(null);
    } else {
      setOperators([]);
      setAvailable(false);
      setReason(r.error);
    }
  }, []);

  useEffect(() => {
    void load();
    const id = setInterval(() => void load(), POLL_MS);
    return () => clearInterval(id);
  }, [load]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return operators;
    return operators.filter(
      (o) =>
        o.name.toLowerCase().includes(q) ||
        o.category.toLowerCase().includes(q) ||
        String(o.id).includes(q),
    );
  }, [operators, query]);

  const enabledCount = useMemo(() => operators.filter((o) => o.available).length, [operators]);

  const onToggle = useCallback(
    async (op: MathOperator, next: boolean) => {
      if (!hasAdminSession()) {
        setHasSession(false);
        setNotice("Login required: unlock an admin session at /killswitch first.");
        return;
      }
      setToggling(op.id);
      setNotice(null);
      const res = await toggleMathOperator(op.id, next, adminToken, actor);
      setToggling(null);
      if (res.ok) {
        setOperators((prev) => prev.map((x) => (x.id === op.id ? { ...x, available: next } : x)));
        setNotice(`${next ? "Enabled" : "Disabled"} ${op.name}`);
      } else {
        setNotice(`Toggle failed for ${op.name}: ${res.error}`);
      }
    },
    [adminToken, actor],
  );

  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm text-muted-foreground">
          {available
            ? `${enabledCount} enabled · ${operators.length} topological operators`
            : "math-engine unavailable"}
        </p>
        <div className="flex items-center gap-2">
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search operator / category…"
            className="h-8 w-64 text-xs"
          />
          <Button variant="outline" size="sm" onClick={() => void load()}>
            Refresh
          </Button>
        </div>
      </div>

      {notice && <p className="text-xs font-mono text-muted-foreground">{notice}</p>}
      {!hasSession && (
        <p className="text-xs font-mono text-warning">
          No admin session — toggles disabled. <a href="/killswitch" className="underline">Unlock at /killswitch</a>
        </p>
      )}

      {available === false && (
        <Card>
          <CardContent className="py-8 text-center">
            <p className="font-mono text-xs uppercase tracking-widest text-muted-foreground">
              math-engine unavailable — {reason}
            </p>
            <p className="mt-2 text-sm text-muted-foreground">
              The math-engine service is not reachable (down or not deployed).
              R8 fail-honest: no operators fabricated.
            </p>
          </CardContent>
        </Card>
      )}

      <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
        {filtered.map((op) => {
          const stratCount = operatorStrategyCount(op.id, STRATEGY_MAPPING);
          return (
            <Card key={op.id}>
              <CardContent className="space-y-2 py-4">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="truncate font-medium" title={op.name}>
                      <span className="font-mono text-xs text-muted-foreground mr-2">#{op.id}</span>
                      {op.name}
                    </div>
                    <div className="text-xs text-muted-foreground">{op.category}</div>
                  </div>
                  <Switch
                    checked={op.available}
                    disabled={!hasSession || toggling === op.id}
                    onCheckedChange={(next) => void onToggle(op, next)}
                    title={
                      !hasSession
                        ? "Admin session required"
                        : op.available
                          ? "Disable this operator in the math-engine runtime"
                          : "Enable this operator in the math-engine runtime"
                    }
                  />
                </div>
                <div className="flex flex-wrap gap-1 pt-1">
                  <Badge
                    variant="outline"
                    className={`text-[10px] font-bold ${
                      op.available
                        ? "bg-success/15 text-success border-success/40"
                        : "bg-muted text-muted-foreground border-border"
                    }`}
                  >
                    {op.available ? "enabled" : "disabled"}
                  </Badge>
                  {stratCount > 0 && (
                    <Badge
                      variant="outline"
                      className="text-[10px]"
                      title={`This operator applies to ${stratCount} of the 264 strategies (264×31 mapping)`}
                    >
                      {stratCount} strategies
                    </Badge>
                  )}
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
      {available && filtered.length === 0 && (
        <p className="py-8 text-center text-sm text-muted-foreground italic">
          No operators match the current filter.
        </p>
      )}
    </div>
  );
}
