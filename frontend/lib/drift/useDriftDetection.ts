"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import type { DriftReport } from "./types";
import { getApiBaseUrl } from "@/lib/api-client";

/**
 * Milliseconds before a single drift check is considered timed out.
 */
const DRIFT_CHECK_TIMEOUT_MS = 8_000;

/**
 * Parses the HTTP response body safely, returning `null` on any parse error.
 */
async function parseBody(res: Response): Promise<unknown | null> {
  try {
    return (await res.json()) as unknown;
  } catch {
    return null;
  }
}

/**
 * Hook that polls the drift-detection endpoint and surfaces cross-layer
 * inconsistencies between PostgreSQL, Redis, and runtime.
 *
 * The check runs immediately on mount and then repeats every
 * `pollIntervalMs`.  A timeout protects each individual fetch so a
 * hanging request does not starve the UI.
 *
 * @param pollIntervalMs - Polling interval in milliseconds (default 30 000).
 *
 * @example
 * ```tsx
 * const { report, isLoading, checkDrift } = useDriftDetection(15_000);
 *
 * if (report?.status === "drift_detected") {
 *   report.differences.forEach(d =>
 *     console.warn(`${d.entity_type}:${d.entity_id} drifted`)
 *   );
 * }
 * ```
 */
export function useDriftDetection(pollIntervalMs = 30_000) {
  const [report, setReport] = useState<DriftReport | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [lastError, setLastError] = useState<string | null>(null);

  /** AbortController for the in-flight request — cleaned up on unmount. */
  const abortRef = useRef<AbortController | null>(null);

  const checkDrift = useCallback(async () => {
    // Cancel any previous in-flight request
    if (abortRef.current) {
      abortRef.current.abort();
    }
    const ctrl = new AbortController();
    abortRef.current = ctrl;

    setIsLoading(true);
    setLastError(null);

    const timer = setTimeout(() => ctrl.abort(), DRIFT_CHECK_TIMEOUT_MS);

    try {
      const res = await fetch(`${getApiBaseUrl()}/api/v1/drift/status`, {
        headers: { accept: "application/json" },
        credentials: "include",
        signal: ctrl.signal,
      });

      clearTimeout(timer);

      if (!res.ok) {
        const body = await parseBody(res);
        const message =
          typeof body === "object" &&
          body !== null &&
          "error" in body &&
          typeof (body as Record<string, unknown>).error === "string"
            ? (body as Record<string, string>).error
            : `HTTP ${res.status}`;
        setLastError(message);
        setReport(null);
        return;
      }

      const data = (await parseBody(res)) as DriftReport | null;

      if (data === null) {
        setLastError("Invalid JSON from drift endpoint");
        setReport(null);
        return;
      }

      // Defensive: normalise empty differences to an array
      setReport({
        ...data,
        differences: Array.isArray(data.differences) ? data.differences : [],
      });
      setLastError(null);
    } catch (err) {
      clearTimeout(timer);
      if (err instanceof Error && err.name === "AbortError") {
        setLastError("Drift check timed out");
      } else {
        setLastError(err instanceof Error ? err.message : "Unknown drift-check error");
      }
      setReport(null);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    // Initial check on mount
    void checkDrift();

    const id = setInterval(() => {
      void checkDrift();
    }, pollIntervalMs);

    return () => {
      clearInterval(id);
      if (abortRef.current) {
        abortRef.current.abort();
      }
    };
  }, [checkDrift, pollIntervalMs]);

  return { report, isLoading, lastError, checkDrift } as const;
}
