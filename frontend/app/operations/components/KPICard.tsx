/**
 * Sprint 3 Task 3.4 — KPI card primitive.
 *
 * Pure presentational. Caller decides if value is "positive" (emerald) or
 * "negative" (amber). No internal heuristics so different KPIs (CV, VAC,
 * TCPI) can express direction independently.
 */
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

interface Props {
  title: string;
  value: string;
  hint: string;
  positive: boolean;
}

export function KPICard({ title, value, hint, positive }: Props) {
  const tone = positive ? "text-emerald-500" : "text-amber-500";
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium text-muted-foreground">{title}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className={`font-mono text-2xl tabular-nums ${tone}`}>{value}</div>
        <p className="text-xs text-muted-foreground mt-1">{hint}</p>
      </CardContent>
    </Card>
  );
}
