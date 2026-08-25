"use client";

/**
 * =============================================================================
 * RuntimeSettingState — FE-0005 (FE-MASTER §3/§14/§64)
 * =============================================================================
 *
 * Reusable one-line setting state: configured → effective (mono), version
 * chips, and an honest lifecycle badge driven by the REAL Runtime ACK wire
 * (useRuntimeAckSocket, event_id bijection). Every state explains itself via
 * title/RUNTIME_SETTING_STATE_COPY (§40 gate explainability).
 *
 * Consumption contract (agreed with d9, EMIT-04): on ACK receipt this component
 * records the payload into the Omni-Store RuntimeAckSlice (global posture
 * feed for FE-0009) and fires onAckApplied so the SURFACE refetches its
 * effective/version props — the component never fetches on its own.
 *
 * Props shape mirrors .ai-work/FE-P3-P4-DOMAIN-SHAPES.md §5.
 */

// SSR-test support (repo pattern, cf. TokenAllowlistTab): the node test
// transformer's classic JSX path needs the React namespace in module scope —
// inert for the Next automatic-runtime app build. Added by d9 (FE-0016 lane)
// so UniverseSaveCoherency/QuoteWeightsCoherency can SSR-render this
// component; behavior unchanged.
import * as React from "react";
import { useEffect, useRef, useState } from "react";
import {
  CheckCircle2,
  Clock3,
  MinusCircle,
  ShieldAlert,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import {
  RUNTIME_SETTING_STATE_COPY,
  deriveRuntimeSettingState,
  type RuntimeSettingAckOutcome,
  type RuntimeSettingRenderState,
  type RuntimeSettingVersion,
} from "@/lib/statemachine/runtimeSettingState";
import { useRuntimeAckSocket } from "@/lib/statemachine/useRuntimeAckSocket";
import { useOmniStore } from "@/lib/store/omni-store";

type Tone = {
  icon: LucideIcon;
  /** static class strings so Tailwind's JIT keeps them */
  chip: string;
  text: string;
};

const TONES: Record<RuntimeSettingRenderState, Tone> = {
  EFFECTIVE: ok(),
  VERIFIED: ok(),
  APPLIED: ok(),
  WAITING_RUNTIME_ACK: warn(Clock3),
  CONFIGURED: info(Clock3),
  NOT_EXPOSED: muted(),
  REJECTED: bad(XCircle),
  TIMEOUT: bad(XCircle),
  DRIFT: bad(ShieldAlert),
};

function ok(): Tone {
  return {
    icon: CheckCircle2,
    chip: "border-primary/30 bg-primary/15 text-primary",
    text: "text-primary",
  };
}
function warn(icon: LucideIcon): Tone {
  return {
    icon,
    chip: "border-warning/30 bg-warning/15 text-warning",
    text: "text-warning",
  };
}
function info(icon: LucideIcon): Tone {
  return {
    icon,
    chip: "border-info/30 bg-info/15 text-info",
    text: "text-info",
  };
}
function muted(): Tone {
  return {
    icon: MinusCircle,
    chip: "border-border bg-muted/70 text-muted-foreground",
    text: "text-muted-foreground",
  };
}
function bad(icon: LucideIcon): Tone {
  return {
    icon,
    chip: "border-destructive/30 bg-destructive/15 text-destructive",
    text: "text-destructive",
  };
}

export interface RuntimeSettingStateProps {
  label: string;
  /** What the operator persisted (PUT body / form state). */
  configured: string | number;
  /** What the runtime reports; null = not computed/served (R8 → "—"). */
  effective: string | number | null;
  /** Version pair when the surface has one (e.g. universe_version). */
  version?: RuntimeSettingVersion;
  /** event_id of the pending mutation; null = steady state (socket off). */
  ackEventId: string | null;
  /** Fired on ACK receipt — the surface refetches its effective/version props. */
  onAckApplied?: () => void;
  className?: string;
}

export function RuntimeSettingState({
  label,
  configured,
  effective,
  version,
  ackEventId,
  onAckApplied,
  className = "",
}: RuntimeSettingStateProps) {
  const [ack, setAck] = useState<RuntimeSettingAckOutcome>({ kind: "none" });
  const recordAck = useOmniStore((s) => s.recordAck);
  const onAckAppliedRef = useRef(onAckApplied);
  onAckAppliedRef.current = onAckApplied;

  // A NEW event_id resets the lifecycle (a second mutation superseded the first).
  useEffect(() => {
    setAck(ackEventId ? { kind: "waiting" } : { kind: "none" });
  }, [ackEventId]);

  useRuntimeAckSocket({
    eventId: ackEventId,
    onAck: (payload) => {
      // Hook contract: fires for status applied|received — both mean the
      // persistence layer confirmed (layer field says which one).
      recordAck(payload);
      setAck({ kind: "applied" });
      onAckAppliedRef.current?.();
    },
    onNack: (reason, payload) => {
      if (payload) recordAck(payload);
      setAck(reason === "timeout" ? { kind: "timeout" } : { kind: "rejected" });
    },
  });

  const state = deriveRuntimeSettingState({ configured, effective, version, ack });
  const copy = RUNTIME_SETTING_STATE_COPY[state];
  const tone = TONES[state];
  const Icon = tone.icon;

  return (
    <div
      role="status"
      className={`inline-flex max-w-full flex-wrap items-center gap-x-2 gap-y-1 font-mono text-[11px] ${className}`}
    >
      <span className="truncate font-sans text-xs font-medium text-foreground/80">
        {label}
      </span>
      <span className="rounded border border-border bg-muted/70 px-1.5 py-0.5 text-foreground/80">
        {String(configured)}
      </span>
      <span aria-hidden className="text-muted-foreground">
        →
      </span>
      <span className="rounded border border-border bg-muted/70 px-1.5 py-0.5 text-foreground/80">
        {effective === null ? "—" : String(effective)}
      </span>
      {version && (
        <span
          className="rounded border border-border bg-muted/40 px-1.5 py-0.5 text-muted-foreground"
          title="configured version → effective version (null = not served)"
        >
          v{version.configured}
          {version.effective === null ? "→—" : `→${version.effective}`}
        </span>
      )}
      <span
        title={copy.hint}
        className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] ${tone.chip}`}
      >
        <Icon size={11} strokeWidth={2.4} className={tone.text} aria-hidden />
        {copy.label}
      </span>
    </div>
  );
}

export default RuntimeSettingState;
