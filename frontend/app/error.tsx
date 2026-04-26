"use client";

import { useEffect } from "react";
import { AlertCircleIcon, RefreshCwIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    if (typeof console !== "undefined") {
      console.error("[arbx] page error:", error);
    }
  }, [error]);

  return (
    <>
      <PageHeader
        title="Something broke"
        lede="A page-level error escaped its handler. The console is still safe — only this view is affected."
      />
      <FocusOnMount>
        <Alert variant="destructive" className="mb-6">
          <AlertCircleIcon />
          <AlertTitle>{error.name || "Error"}</AlertTitle>
          <AlertDescription>
            <code className="rounded bg-destructive/10 px-1.5 py-0.5 font-mono text-xs">
              {error.message || "no message"}
            </code>
            {error.digest && (
              <div className="mt-2 text-xs text-muted-foreground">
                digest: <code className="font-mono">{error.digest}</code>
              </div>
            )}
            <p className="mt-3 text-sm">
              Reload the view, or go back to the operator console index. We never
              synthesize values to hide a fault.
            </p>
          </AlertDescription>
        </Alert>
      </FocusOnMount>

      <div className="flex flex-wrap gap-2">
        <Button type="button" onClick={reset}>
          <RefreshCwIcon /> Try again
        </Button>
        <Button type="button" variant="outline" asChild>
          <a href="/">Back to console</a>
        </Button>
      </div>
    </>
  );
}
