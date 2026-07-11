"use client";

import { ShieldCheckIcon } from "lucide-react";

export function AuditLogsEmpty() {
  return (
    <div className="flex flex-col items-center justify-center rounded-xl border border-dashed p-8 text-center animate-in fade-in slide-in-from-bottom-4">
      <div className="flex size-12 items-center justify-center rounded-full bg-muted">
        <ShieldCheckIcon className="size-6 text-muted-foreground" />
      </div>
      <h3 className="mt-4 font-semibold">No audit logs found</h3>
      <p className="mt-1 max-w-sm text-sm text-muted-foreground">
        No administrative actions matching the given filters were found in the audit log.
      </p>
    </div>
  );
}
