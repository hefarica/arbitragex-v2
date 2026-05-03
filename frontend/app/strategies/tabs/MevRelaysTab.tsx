/**
 * Sprint 2 Task 2.3 — MEV Services tab.
 *
 * Read-only display of relay configuration. The `relays` table (migration
 * 013) has its own admin endpoint already; this tab is a pointer to the
 * existing /config page section to avoid duplicating mutation surface.
 */
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function MevRelaysTab() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>MEV Services</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3 text-sm text-muted-foreground">
        <p>
          MEV relay configuration (Flashbots Protect / MEV Blocker / Titan / BloxRoute / Eden)
          lives in the <code className="font-mono text-xs">relays</code> table (migration 013)
          with its own admin endpoints under <code className="font-mono text-xs">/admin/relays</code>.
        </p>
        <p>
          Use the existing <a href="/config" className="underline">Current config</a> page →
          Relays section to inspect, or call{" "}
          <code className="font-mono text-xs">/admin/relays</code> directly. A merged Relay
          editor inside this panel is queued for Sprint 5 (relay-priority surfacing).
        </p>
      </CardContent>
    </Card>
  );
}
