/**
 * OMEGA-8 / M5 Capa 4 Frontend Hardening — Fail-Honest Frontier State View.
 *
 * Renders the discriminated union returned by `getValidated()` so every page
 * shows the operator EXACTLY what happened upstream:
 *   - "ENDPOINT_NOT_IMPLEMENTED" (404)
 *   - "AUTH_REQUIRED"
 *   - "UNAVAILABLE" (network / 5xx)
 *   - "INVALID_RESPONSE" (schema drift)
 *   - "EMPTY" (200 OK with zero rows — distinct from caída).
 *
 * Pages compose this between a "loading" guard and their data table; the
 * goal is to make it impossible to silently render `rows: []` when the
 * endpoint returned 404.
 */

import { AlertCircleIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

export interface FrontierStateMessage {
  kind: "loading" | "auth_required" | "unavailable" | "invalid_response" | "endpoint_not_implemented" | "empty";
  detail?: string;
  resource: string;
}

export function FrontierStateView({ msg }: { msg: FrontierStateMessage }) {
  if (msg.kind === "loading") {
    return (
      <div className="mt-4 rounded-md border border-border/50 bg-muted/40 p-4 text-sm text-muted-foreground">
        Loading {msg.resource}…
      </div>
    );
  }
  if (msg.kind === "empty") {
    return (
      <div className="mt-4 rounded-md border border-dashed border-border p-4 text-sm text-muted-foreground">
        No {msg.resource} rows returned by the backend. This is a valid empty state — the endpoint responded with an
        empty list (not a failure).
      </div>
    );
  }
  const variant: "default" | "destructive" = msg.kind === "endpoint_not_implemented" ? "default" : "destructive";
  const titleMap: Record<"auth_required" | "unavailable" | "invalid_response" | "endpoint_not_implemented", string> = {
    auth_required: `${msg.resource}: authentication required`,
    unavailable: `${msg.resource}: backend unavailable`,
    invalid_response: `${msg.resource}: invalid response`,
    endpoint_not_implemented: `${msg.resource}: endpoint not implemented`,
  };
  return (
    <Alert variant={variant} className="mt-4">
      <AlertCircleIcon />
      <AlertTitle>{titleMap[msg.kind]}</AlertTitle>
      <AlertDescription>
        <code className="font-mono text-xs">{msg.detail ?? "(no detail)"}</code>
      </AlertDescription>
    </Alert>
  );
}
