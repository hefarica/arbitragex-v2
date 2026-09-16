"use client";

/**
 * AiAgentBanner.tsx — Top-of-terminal banner indicating AI agent status,
 * mirroring the DeFiBot "AI Trading Agent · Idle / Active" badge but driven
 * by spine-truth: the simulator hot-path flag and the execution_worker
 * watchdog. Each `agent_*` flag maps 1:1 to a spine subsystem so the
 * operator always sees the real state, never marketing.
 *
 * R8 fail-honest:
 *   - When all flags are false, the banner reads "Idle · Spine gated" — no
 *     pretense of activity.
 *   - When `agent_executor` is true but `agent_simulator` is false, the
 *     banner warns "Executor armed without simulator — fail-closed".
 */

import React from "react";
import { Activity, CircleAlert, CircleCheck } from "lucide-react";

export interface AgentStatus {
  /** prioritization-spine simulator hot-path is ON */
  agent_simulator: boolean;
  /** execution_worker is spawned & alive */
  agent_executor: boolean;
  /** notifier (Telegram/Discord/PagerDuty) wired */
  agent_notifier: boolean;
  /** risk-gate circuit breaker subscribed */
  agent_breaker: boolean;
}

export interface AiAgentBannerProps {
  status: AgentStatus;
}

function summary(status: AgentStatus): {
  label: string;
  tone: "ok" | "warn" | "idle";
  detail: string;
} {
  const { agent_simulator, agent_executor, agent_notifier, agent_breaker } = status;
  const active = [
    agent_simulator,
    agent_executor,
    agent_notifier,
    agent_breaker,
  ].filter(Boolean).length;

  if (agent_executor && !agent_simulator) {
    return {
      label: "Misconfigured · Executor without Simulator",
      tone: "warn",
      detail:
        "Spine gates will fail-close every bundle until SIMULATOR_HOT_PATH is enabled.",
    };
  }
  if (active === 4) {
    return {
      label: "Active · 4/4 subsystems online",
      tone: "ok",
      detail: "Spine fully wired. Watching mempool for qualifying opportunities.",
    };
  }
  if (active === 0) {
    return {
      label: "Idle · Spine gated",
      tone: "idle",
      detail: "No subsystems online. Enable simulator + executor to begin scoring.",
    };
  }
  return {
    label: `Partial · ${active}/4 subsystems online`,
    tone: "warn",
    detail:
      "Operating with reduced safety surface. Bring missing subsystems online before mainnet capital.",
  };
}

export function AiAgentBanner({ status }: AiAgentBannerProps) {
  const s = summary(status);
  const toneCls =
    s.tone === "ok"
      ? "border-success/40 bg-success/10 text-success"
      : s.tone === "warn"
      ? "border-warning/50 bg-warning/10 text-warning"
      : "border-border bg-muted/40 text-muted-foreground";
  const Icon =
    s.tone === "ok" ? CircleCheck : s.tone === "warn" ? CircleAlert : Activity;

  return (
    <section
      data-testid="ai-agent-banner"
      data-tone={s.tone}
      aria-live="polite"
      className={["flex items-center gap-3 rounded-xl border p-3", toneCls].join(" ")}
    >
      <span
        className={[
          "relative inline-flex h-2.5 w-2.5 shrink-0 rounded-full",
          s.tone === "ok"
            ? "bg-success"
            : s.tone === "warn"
            ? "bg-warning"
            : "bg-muted-foreground",
        ].join(" ")}
        aria-hidden
      >
        {s.tone === "ok" ? (
          <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-success/60" />
        ) : null}
      </span>
      <Icon className="h-4 w-4 shrink-0" aria-hidden />
      <div className="flex flex-1 flex-col">
        <span className="text-sm font-semibold leading-tight">{s.label}</span>
        <span className="text-[11px] opacity-90">{s.detail}</span>
      </div>
      <ul className="hidden gap-3 text-[11px] sm:flex" aria-label="Subsystem checklist">
        <li
          className={status.agent_simulator ? "text-success" : "text-muted-foreground/70"}
          title="prioritization-spine simulator hot-path"
        >
          ● simulator
        </li>
        <li
          className={status.agent_executor ? "text-success" : "text-muted-foreground/70"}
          title="execution_worker spawned"
        >
          ● executor
        </li>
        <li
          className={status.agent_notifier ? "text-success" : "text-muted-foreground/70"}
          title="notifier wired"
        >
          ● notifier
        </li>
        <li
          className={status.agent_breaker ? "text-success" : "text-muted-foreground/70"}
          title="circuit breaker"
        >
          ● breaker
        </li>
      </ul>
    </section>
  );
}
