"use client";

// FE-8: Load More pagination for executions.
//
// Strategy: The edge exposes GET /api/executions/recent?limit=N with no cursor/offset
// param yet. Until the backend adds offset support we re-fetch with an increasing limit.
// This is honest (R8): we don't fabricate data; we just ask for a larger window.
// When the api-server exposes ?cursor= or ?offset=, swap out fetchPage() below.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { ExecutionsTable } from "@/features/executions/ExecutionsTable";
import { ExecutionsEmpty } from "@/features/executions/ExecutionsEmpty";
import type { ExecutionRow } from "@/lib/api-client";
import { getApiBaseUrl } from "@/lib/api-client";

const PAGE_SIZE = 50;

interface Props {
  initialItems: ExecutionRow[];
  initialTotal: number;
}

export function ExecutionsClient({ initialItems, initialTotal }: Props) {
  const [items, setItems] = useState<ExecutionRow[]>(initialItems);
  const [limit, setLimit] = useState<number>(initialTotal);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // mayHaveMore: true when the last fetch returned exactly limit rows (server might have more).
  const [mayHaveMore, setMayHaveMore] = useState<boolean>(initialItems.length >= initialTotal);

  const loadMore = async () => {
    const nextLimit = limit + PAGE_SIZE;
    setLoading(true);
    setError(null);
    try {
      const base = getApiBaseUrl();
      const res = await fetch(`${base}/api/executions/recent?limit=${nextLimit}`, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(5000),
        // cache: "no-store" is not valid in browser fetch; omit for browser calls.
      });
      if (!res.ok) {
        setError(`edge HTTP ${res.status}`);
        return;
      }
      const json = await res.json() as { items?: ExecutionRow[]; count?: number };
      const fetched = Array.isArray(json.items) ? json.items : [];
      setItems(fetched);
      setLimit(nextLimit);
      // If we got fewer rows than requested, we've reached the end.
      setMayHaveMore(fetched.length >= nextLimit);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  };

  if (items.length === 0) {
    return <ExecutionsEmpty />;
  }

  return (
    <div className="space-y-4">
      <ExecutionsTable items={items} />
      {error && (
        <p className="text-sm text-destructive font-mono">{error}</p>
      )}
      {mayHaveMore && (
        <div className="flex justify-center">
          <Button
            variant="outline"
            onClick={loadMore}
            disabled={loading}
          >
            {loading ? "Loading…" : `Load more (showing ${items.length})`}
          </Button>
        </div>
      )}
    </div>
  );
}
