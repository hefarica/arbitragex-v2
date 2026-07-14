/**
 * useGatesStatus — Poll backend gate metrics endpoint for frontend visualization.
 */

"use client";

import { useEffect, useRef, useState } from "react";

interface GateCheckpoint {
  gate_id: string;
  gate_label: string;
  status: "passed" | "failed" | "fired" | "blocked";
  gate_score?: number;
  reason: string;
}

interface GateMetrics {
  gates: GateCheckpoint[];
  summary: {
    total: number;
    passed: number;
    failed: number;
    fired: number;
    blocked: number;
    average_score: number | null;
  };
  generated_at: string;
}

interface UseGatesStatusResult {
  metrics: GateMetrics | null;
  loading: boolean;
}

export function useGatesStatus({ edgeUrl }: { edgeUrl?: string }): UseGatesStatusResult {
  const [metrics, setMetrics] = useState<GateMetrics | null>(null);
  const [loading, setLoading] = useState(true);
  const pollingRef = useRef(false);

  useEffect(() => {
    if (!edgeUrl || pollingRef.current) return;

    pollingRef.current = true;

    const fetchData = async () => {
      try {
        const response = await fetch(`${edgeUrl}/api/gates/status`);
        if (response.ok) {
          const data = await response.json();
          setMetrics(data.metrics || null);
        }
      } catch (error) {
        console.error("Failed to fetch gate metrics:", error);
      } finally {
        setLoading(false);
        pollingRef.current = false;
      }
    };

    fetchData();

    const interval = setInterval(fetchData, 3000);

    return () => clearInterval(interval);
  }, [edgeUrl]);

  return { metrics, loading };
}

export type { GateCheckpoint, GateMetrics };
