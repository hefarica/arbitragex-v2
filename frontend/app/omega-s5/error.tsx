"use client";

/**
 * OMEGA-8 / M5 Capa 4 Fase 13 (P2-LOG-1) — OMEGA-S5 section error boundary.
 *
 * Caches a crash inside any /omega-s5/* page so the global shell stays alive.
 * Fail-Honest: we surface the error digest (server-generated, not the raw
 * stack which may leak file paths) and a Retry button.
 */

import { useEffect } from "react";
import { AlertCircleIcon, RefreshCwIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

export default function OmegaS5Error({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    if (typeof console !== "undefined" && typeof console.error === "function") {
      console.error("[omega-s5/error.tsx]", error.message, error.digest ?? "(no digest)");
    }
  }, [error]);

  return (
    <Alert variant="destructive" className="my-6">
      <AlertCircleIcon />
      <AlertTitle>OMEGA-S5 section error</AlertTitle>
      <AlertDescription>
        <p className="mb-2 text-sm">
          A component below this boundary threw an exception during render. The rest of the operator console is
          unaffected.
        </p>
        {error.digest && (
          <p className="text-xs font-mono">digest: {error.digest}</p>
        )}
        <Button onClick={reset} variant="outline" size="sm" className="mt-3">
          <RefreshCwIcon className="mr-2 size-4" />
          Retry
        </Button>
      </AlertDescription>
    </Alert>
  );
}
