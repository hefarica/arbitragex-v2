/**
 * AgentTeamsPanel SSR-only static markup tests.
 *
 * useEffect doesn't fire under renderToStaticMarkup, so we see only the
 * initial "loading" render. Assertions:
 *   - "Agent Teams" title present
 *   - "17 agents" badge present (workspace_verified count)
 *   - "Loading agent teams status" copy present
 *   - The R8 source disclaimer is visible
 *   - NO "all green" or "GO live" text appears at initial render
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { AgentTeamsPanel } from "../AgentTeamsPanel";

const html = renderToStaticMarkup(<AgentTeamsPanel />);

describe("AgentTeamsPanel (SSR initial state)", () => {
  it("renders Agent Teams title", () => {
    expect(html).toMatch(/Agent Teams/);
  });

  it("renders the '17 agents' workspace-verified badge", () => {
    expect(html).toMatch(/17 agents/);
    expect(html).toMatch(/workspace-verified/);
  });

  it("renders R8 disclaimer about source types", () => {
    expect(html).toMatch(/workspace_verified/);
    expect(html).toMatch(/runtime/);
    expect(html).toMatch(/demotes/i);
  });

  it("renders loading state initially", () => {
    expect(html).toMatch(/Loading agent teams status/i);
  });

  it("regression: never renders GO live or all-green claim at initial render", () => {
    expect(html).not.toMatch(/all (agents )?green/i);
    expect(html).not.toMatch(/GO live(?! NO)/i);
    expect(html).not.toMatch(/Live trading: ON/);
  });
});
