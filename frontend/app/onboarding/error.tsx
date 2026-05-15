"use client";

/**
 * OMEGA-8 / M5 Capa 4 Fase 13 — Onboarding section error boundary.
 */

import { useEffect } from "react";
import { AlertCircleIcon, RefreshCwIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

export default function OnboardingError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    if (typeof console !== "undefined" && typeof console.error === "function") {
      console.error("[onboarding/error.tsx]", error.message, error.digest ?? "(no digest)");
    }
  }, [error]);

  return (
    <Alert variant="destructive" className="my-6">
      <AlertCircleIcon />
      <AlertTitle>Onboarding error</AlertTitle>
      <AlertDescription>
        <p className="mb-2 text-sm">
          An onboarding phase component crashed. Retry below; if the failure persists, check the api-server logs and
          retry from the previous phase.
        </p>
        {error.digest && <p className="text-xs font-mono">digest: {error.digest}</p>}
        <Button onClick={reset} variant="outline" size="sm" className="mt-3">
          <RefreshCwIcon className="mr-2 size-4" />
          Retry
        </Button>
      </AlertDescription>
    </Alert>
  );
}
