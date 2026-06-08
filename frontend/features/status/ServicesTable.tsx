import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import type { StatusResponse } from "@/lib/api-client";

/**
 * ITER-18 / OMEGA-S5 — honest service health rendering.
 *
 * Tripartite status (must match backend pingUpstream + StatusResponseSchema):
 *   - UP        : v.ok === true                  → success badge
 *   - DEGRADED  : v.ok === false && v.degraded   → warning badge, machine `reason` exposed
 *   - DOWN      : v.ok === false && !v.degraded  → destructive badge, raw detail surfaced
 *
 * Detail column rule:
 *   - DEGRADED  → human label ("no RPC configured"), never "fetch failed"
 *   - UP        → "HTTP {status}"
 *   - DOWN      → upstream detail or "—"
 *
 * Row keys carry data-testid="service-health-row-{name}" so Playwright can
 * disambiguate this *health* table from the *control* table in
 * ServiceControlPanel (both share the "searcher-rs" text, which triggered
 * strict-mode violations in CI).
 */
export function ServicesTable({ services }: { services: StatusResponse["services"] }) {
  return (
    <Card className="py-0">
      <CardContent className="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Service</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Detail</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {Object.entries(services).map(([name, v]) => {
              const isUp = v.ok;
              const isDegraded = !v.ok && v.degraded === true;
              const isDown = !v.ok && !isDegraded;
              const label = isUp ? "UP" : isDegraded ? "DEGRADED" : "DOWN";
              const badgeVariant: "success" | "warning" | "destructive" = isUp
                ? "success"
                : isDegraded
                  ? "warning"
                  : "destructive";
              const dotClass = isUp ? "bg-success" : isDegraded ? "bg-warning" : "bg-destructive";

              // Detail rendering: when degraded we always show the human label
              // (never the raw "fetch failed" message), so the test contract
              // `not.toContainText(/fetch failed/i)` holds end-to-end.
              const detailText = isDegraded
                ? (v.detail ?? "no RPC configured")
                : v.status
                  ? `HTTP ${v.status}`
                  : (v.detail ?? "—");

              return (
                <TableRow
                  key={name}
                  data-testid={`service-health-row-${name}`}
                  data-service={name}
                  data-status={label.toLowerCase()}
                  data-reason={v.reason ?? ""}
                >
                  <TableCell className="font-mono text-sm">{name}</TableCell>
                  <TableCell>
                    <Badge variant={badgeVariant}>
                      <span className={`size-1.5 rounded-full ${dotClass}`} aria-hidden />
                      {label}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-muted-foreground">{detailText}</TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}
