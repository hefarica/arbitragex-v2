// frontend/components/__tests__/ControlScopeBadge.test.tsx
//
// FE-0041 (§52-§54 §63) — the scope label: three honest kinds, each with its
// explainability copy (§63). Also pins the §53 contract on /settings: every
// card carries LOCAL_PREFS and the page contains ZERO RUNTIME_MUTATION
// labels (settings is presentation scope by contract).
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { ControlScopeBadge } from "../ControlScopeBadge";

function badge(kind: Parameters<typeof ControlScopeBadge>[0]["kind"]): string {
  return renderToStaticMarkup(React.createElement(ControlScopeBadge, { kind }));
}

describe("ControlScopeBadge (§63)", () => {
  it("VIEW_ONLY: outline tone, explains zero writes", () => {
    const html = badge("VIEW_ONLY");
    expect(html).toContain("VIEW_ONLY");
    expect(html).toContain('data-testid="control-scope-VIEW_ONLY"');
    expect(html).toContain("cero escrituras");
  });

  it("LOCAL_PREFS: info tone, explains localStorage-only (§53)", () => {
    const html = badge("LOCAL_PREFS");
    expect(html).toContain("LOCAL_PREFS");
    expect(html).toContain("SOLO localStorage");
    expect(html).toContain("jamás muta runtime");
    expect(html).toContain("§53");
  });

  it("RUNTIME_MUTATION: warning tone, names the SSOT write plane (§54 §56)", () => {
    const html = badge("RUNTIME_MUTATION");
    expect(html).toContain("RUNTIME_MUTATION");
    expect(html).toContain("plano de config");
    expect(html).toContain("putTradingConfig");
    expect(html).toContain("§54");
  });

  it("the three kinds are visually distinct (variant classes differ)", () => {
    const a = badge("VIEW_ONLY");
    const b = badge("LOCAL_PREFS");
    const c = badge("RUNTIME_MUTATION");
    expect(a).not.toBe(b);
    expect(b).not.toBe(c);
    expect(a).not.toBe(c);
  });

  it("R1: pure render is byte-identical across invocations", () => {
    expect(badge("RUNTIME_MUTATION")).toBe(badge("RUNTIME_MUTATION"));
  });
});

describe("/settings §53 contract (presentation scope)", () => {
  it("every card is labeled LOCAL_PREFS and none is RUNTIME_MUTATION", async () => {
    // Dynamic import: SettingsClient is a client component using sonner/lucide;
    // SSR render is R1-safe (isMounted=false → disabled form, no localStorage).
    const { SettingsClient } = await import("@/app/settings/SettingsClient");
    const html = renderToStaticMarkup(React.createElement(SettingsClient));
    const localCount = html.split('data-testid="control-scope-LOCAL_PREFS"').length - 1;
    expect(localCount).toBe(3); // Notifications · Feed & Polling · Display
    expect(html).not.toContain("RUNTIME_MUTATION");
  });
});
