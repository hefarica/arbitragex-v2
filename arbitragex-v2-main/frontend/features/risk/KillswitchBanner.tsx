import { ShieldCheckIcon, ShieldOffIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import type { KillSwitchState } from "@/lib/api-client";
import { fmtDateTime } from "@/lib/formatters";

export function KillswitchBanner({ ks }: { ks: KillSwitchState }) {
  return (
    <Alert variant={ks.enabled ? "destructive" : "success"} className="mb-6">
      {ks.enabled ? <ShieldOffIcon /> : <ShieldCheckIcon />}
      <AlertTitle>
        Kill-switch: {ks.enabled ? "ARMED — executions blocked" : "disabled — executions permitted"}
      </AlertTitle>
      <AlertDescription>
        {ks.reason && <div>reason: <em>{ks.reason}</em></div>}
        <div className="text-xs">
          updated {fmtDateTime(ks.updated_at)}
          {ks.triggered_by && <> · by <code className="font-mono">{ks.triggered_by}</code></>}
        </div>
      </AlertDescription>
    </Alert>
  );
}
