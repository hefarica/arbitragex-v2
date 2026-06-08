"use client";

/**
 * OMEGA-OK validation checklist — interactive client-side.
 *
 * R1 Mounted Snapshot Pattern: receives a fully serializable list of items
 * from the server component and owns the boolean toggle state. No
 * Date.now / window / WebSocket on first render — hydration-safe.
 *
 * State is intentionally NOT persisted: each operator visit starts fresh.
 * Mirroring the PR body's intent — the checklist is a walkthrough, not a
 * source of truth. Real validation comes from CLI verification commands.
 */
import { useMemo, useState } from "react";
import { CheckIcon, RotateCcwIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import type { ChecklistItem } from "./data";

const GROUP_LABELS: Record<ChecklistItem["group"], string> = {
  policy: "Políticas GitHub",
  artifacts: "Artefactos del PR",
  ci: "CI · branch protection",
  deploy: "Deploy productivo",
};

const GROUP_TONE: Record<
  ChecklistItem["group"],
  { ring: string; chip: string }
> = {
  policy: {
    ring: "border-warning/30",
    chip: "bg-warning/10 text-warning border-warning/40",
  },
  artifacts: {
    ring: "border-success/30",
    chip: "bg-success/10 text-success border-success/40",
  },
  ci: { ring: "border-info/30", chip: "bg-info/10 text-info border-info/40" },
  deploy: {
    ring: "border-primary/30",
    chip: "bg-primary/10 text-primary border-primary/40",
  },
};

export function DeployPipelineClient({ checklist }: { checklist: ChecklistItem[] }) {
  const [checked, setChecked] = useState<Record<string, boolean>>({});

  const total = checklist.length;
  const completed = Object.values(checked).filter(Boolean).length;
  const pct = total === 0 ? 0 : Math.round((completed / total) * 100);

  const grouped = useMemo(() => {
    const map = new Map<ChecklistItem["group"], ChecklistItem[]>();
    for (const item of checklist) {
      if (!map.has(item.group)) map.set(item.group, []);
      map.get(item.group)!.push(item);
    }
    return Array.from(map.entries());
  }, [checklist]);

  const allDone = completed === total && total > 0;

  return (
    <Card>
      <CardHeader className="border-b">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <CardTitle className="flex items-center gap-2 text-base">
              OMEGA-OK · {completed}/{total} verificados
              {allDone && (
                <Badge variant="success" className="font-mono text-[10px]">
                  <CheckIcon />
                  ENTREGA OK
                </Badge>
              )}
            </CardTitle>
            <p className="mt-1 text-xs text-muted-foreground">
              Estado local — no persiste. Para auditoría real, los comandos CLI
              listados en cada ítem son la fuente de verdad.
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            disabled={completed === 0}
            onClick={() => setChecked({})}
            className="gap-2"
          >
            <RotateCcwIcon className="size-3.5" />
            Reset
          </Button>
        </div>
        <div className="mt-3 flex items-center gap-3">
          <Progress value={pct} className="h-2 flex-1" />
          <span className="font-mono text-xs tabular-nums text-muted-foreground">
            {pct.toString().padStart(3, " ")}%
          </span>
        </div>
      </CardHeader>
      <CardContent className="pt-6">
        <div className="grid gap-4 lg:grid-cols-2">
          {grouped.map(([group, items]) => {
            const tone = GROUP_TONE[group];
            const groupCompleted = items.filter((it) => checked[it.id]).length;
            return (
              <div key={group} className={`rounded-lg border ${tone.ring} p-3`}>
                <div className="mb-3 flex items-center justify-between">
                  <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
                    {GROUP_LABELS[group]}
                  </span>
                  <Badge variant="outline" className={`font-mono text-[10px] ${tone.chip}`}>
                    {groupCompleted}/{items.length}
                  </Badge>
                </div>
                <ul className="space-y-1.5">
                  {items.map((it) => {
                    const isOn = !!checked[it.id];
                    return (
                      <li key={it.id}>
                        <button
                          type="button"
                          onClick={() =>
                            setChecked((prev) => ({ ...prev, [it.id]: !prev[it.id] }))
                          }
                          className={`group flex w-full items-start gap-2.5 rounded-md border px-2.5 py-2 text-left transition-colors ${
                            isOn
                              ? "border-success/40 bg-success/5"
                              : "border-transparent hover:border-border hover:bg-muted/40"
                          }`}
                        >
                          <span
                            aria-hidden
                            className={`mt-0.5 inline-flex size-4 shrink-0 items-center justify-center rounded border transition-colors ${
                              isOn
                                ? "border-success bg-success text-success-foreground"
                                : "border-muted-foreground/40 bg-background"
                            }`}
                          >
                            {isOn && <CheckIcon className="size-3" />}
                          </span>
                          <span
                            className={`text-xs leading-snug ${
                              isOn ? "text-foreground" : "text-muted-foreground"
                            }`}
                          >
                            {it.label}
                          </span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}
