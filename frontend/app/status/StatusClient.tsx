"use client";

// FE-11: Real-time polling for /status page.
// R1 Mounted Snapshot Pattern:
//   - Server page passes initialStatus as prop.
//   - setInterval lives exclusively inside useEffect.
//   - No Date.now(), window, or document access outside useEffect.

import { useEffect, useState } from "react";
import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { SystemKpiGrid } from "@/features/status/SystemKpiGrid";
import { ServicesTable } from "@/features/status/ServicesTable";
import { getStatus } from "@/lib/api-client";
import type { StatusResponse } from "@/lib/api-client";

const POLL_INTERVAL_MS = 5_000;

interface Props {
  initialStatus: StatusResponse;
}

export function StatusClient({ initialStatus }: Props) {
  const [status, setStatus] = useState<StatusResponse>(initialStatus);
  const [pollError, setPollError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<number>(0);

  // R1: all side-effects inside useEffect — never in render.
  useEffect(() => {
    setLastUpdated(Date.now());
    let alive = true;

    const poll = async () => {
      const res = await getStatus();
      if (!alive) return;
      if (res.ok) {
        setStatus(res.data);
        setPollError(null);
        setLastUpdated(Date.now());
      } else {
        setPollError(res.error);
      }
    };

    const timer = setInterval(() => void poll(), POLL_INTERVAL_MS);

    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  return (
    <div className="space-y-6">
      {pollError && (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>poll error</AlertTitle>
          <AlertDescription>
            <code className="font-mono text-xs">{pollError}</code>
            <p className="mt-1 text-xs">Retrying every {POLL_INTERVAL_MS / 1000}s. Displaying last known data.</p>
          </AlertDescription>
        </Alert>
      )}

      <SystemKpiGrid status={status} />

      <h2>Services</h2>
      <ServicesTable services={status.services} />
    </div>
  );
}
