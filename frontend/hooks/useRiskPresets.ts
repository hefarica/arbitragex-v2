"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  resolvePosture,
  RISK_POSTURE_ORDER,
  type RiskPostureId,
  type ResolvedRiskLimits,
} from "@/lib/presets/riskPresets";

/**
 * Client state for the operator presets page.
 *
 * R1 (Mounted Snapshot): initial state is DETERMINISTIC (capital 0, balanced)
 * so SSR markup and first client render match. localStorage is read only inside
 * useEffect (after mount), never during render — no hydration mismatch.
 *
 * The hook computes risk limits via the pure resolvePosture() and persists the
 * operator's (posture, capital) so a refresh restores their selection. It does
 * NOT write to trading_config — "apply" stages a local draft the operator
 * reviews before the separate admin-authenticated push.
 */

const STORAGE_KEY = "arbx:operator:preset";
const APPLIED_KEY = "arbx:operator:preset:applied";
const DEFAULT_POSTURE: RiskPostureId = "balanced";

export interface AppliedDraft {
  postureId: RiskPostureId;
  capitalUsd: number;
  resolved: ResolvedRiskLimits;
  /** ISO timestamp; set in an event handler (not render) so R1 holds. */
  savedAt: string;
}

interface PersistShape {
  postureId: RiskPostureId;
  capitalUsd: number;
}

function isPostureId(v: unknown): v is RiskPostureId {
  return v === "conservative" || v === "balanced" || v === "aggressive";
}

export interface UseRiskPresets {
  capitalUsd: number;
  setCapitalUsd: (v: number) => void;
  capitalError: string | null;
  selectedPostureId: RiskPostureId;
  selectPosture: (id: RiskPostureId) => void;
  resolvedByPosture: Record<RiskPostureId, ResolvedRiskLimits>;
  apply: () => AppliedDraft | null;
  appliedDraft: AppliedDraft | null;
}

export function useRiskPresets(): UseRiskPresets {
  const [capitalUsd, setCapitalUsdState] = useState<number>(0);
  const [selectedPostureId, setSelectedPostureId] = useState<RiskPostureId>(DEFAULT_POSTURE);
  const [appliedDraft, setAppliedDraft] = useState<AppliedDraft | null>(null);
  const [hydrated, setHydrated] = useState(false);

  // Hydrate from localStorage once, after mount (R1).
  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as Partial<PersistShape>;
        if (isPostureId(parsed.postureId)) setSelectedPostureId(parsed.postureId);
        if (typeof parsed.capitalUsd === "number" && Number.isFinite(parsed.capitalUsd)) {
          setCapitalUsdState(parsed.capitalUsd);
        }
      }
    } catch {
      // malformed storage → start from deterministic defaults (fail-honest)
    }
    setHydrated(true);
  }, []);

  // Persist (posture, capital) after hydration so the first paint's defaults
  // don't clobber a stored value before we've read it.
  useEffect(() => {
    if (!hydrated) return;
    try {
      const payload: PersistShape = { postureId: selectedPostureId, capitalUsd };
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
    } catch {
      // quota / unavailable → non-fatal
    }
  }, [hydrated, selectedPostureId, capitalUsd]);

  const setCapitalUsd = useCallback((v: number) => {
    setCapitalUsdState(Number.isFinite(v) ? v : 0);
  }, []);

  const selectPosture = useCallback((id: RiskPostureId) => {
    setSelectedPostureId(id);
  }, []);

  const resolvedByPosture = useMemo(() => {
    const out = {} as Record<RiskPostureId, ResolvedRiskLimits>;
    for (const id of RISK_POSTURE_ORDER) {
      out[id] = resolvePosture(id, { capitalUsd });
    }
    return out;
  }, [capitalUsd]);

  const capitalError = useMemo(() => {
    if (!Number.isFinite(capitalUsd) || capitalUsd <= 0) {
      return "Enter capital greater than 0. Caps resolve to 0 until capital is configured.";
    }
    return null;
  }, [capitalUsd]);

  const apply = useCallback((): AppliedDraft | null => {
    if (!Number.isFinite(capitalUsd) || capitalUsd <= 0) return null;
    const draft: AppliedDraft = {
      postureId: selectedPostureId,
      capitalUsd,
      resolved: resolvePosture(selectedPostureId, { capitalUsd }),
      savedAt: new Date().toISOString(),
    };
    try {
      window.localStorage.setItem(APPLIED_KEY, JSON.stringify(draft));
    } catch {
      // non-fatal
    }
    setAppliedDraft(draft);
    return draft;
  }, [capitalUsd, selectedPostureId]);

  return {
    capitalUsd,
    setCapitalUsd,
    capitalError,
    selectedPostureId,
    selectPosture,
    resolvedByPosture,
    apply,
    appliedDraft,
  };
}
