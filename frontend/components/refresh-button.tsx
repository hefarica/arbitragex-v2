"use client";

import { useTransition } from "react";
import { useRouter } from "next/navigation";
import { RefreshCwIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export function RefreshButton({ className }: { className?: string }) {
  const router = useRouter();
  const [isPending, startTransition] = useTransition();

  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      disabled={isPending}
      onClick={() => startTransition(() => router.refresh())}
      aria-label="Refresh data from edge"
      className={className}
    >
      <RefreshCwIcon className={cn("size-4", isPending && "animate-spin")} />
      {isPending ? "Refreshing…" : "Refresh"}
    </Button>
  );
}
