// frontend/components/__tests__/NavigationSentinel.test.tsx
//
// BROWSE-FIX-4.6 (2026-09-17) — tests for the navigation sentinel.
//
// Repo pattern (ArchivePanel.test.tsx / AdminSessionBadge.test.tsx): the
// frontend test env is `node` (no jsdom) — the component is SSR-rendered to
// static HTML and must render NOTHING (null component, R1-safe: no window
// access before mount), while the attribution logic is unit-tested as pure
// functions. No fabricated telemetry anywhere (RULE 00): the formatter only
// echoes what a real navigation event would carry.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  formatNavSentinelLine,
  getNavigationApi,
  NavigationSentinel,
} from "../NavigationSentinel";

describe("NavigationSentinel — SSR null-render (R1)", () => {
  it("renders nothing on the server and never touches window at import/render time", () => {
    // renderToStaticMarkup runs the component body synchronously like SSR;
    // useEffect does NOT run here, so any window access would throw in node.
    expect(renderToStaticMarkup(<NavigationSentinel />)).toBe("");
  });
});

describe("formatNavSentinelLine — attribution line", () => {
  it("reports ms proximity when user input was recorded", () => {
    expect(
      formatNavSentinelLine({
        mechanism: "navigation-api:push",
        destination: "https://arbx.example/",
        sinceInputMs: 4120,
      }),
    ).toBe(
      "[arbx] nav-sentinel: navigation-api:push → https://arbx.example/ (4120ms since last user input)",
    );
  });

  it("declares absence of user input honestly (null ≠ 0ms)", () => {
    expect(
      formatNavSentinelLine({
        mechanism: "popstate",
        destination: "/",
        sinceInputMs: null,
      }),
    ).toBe("[arbx] nav-sentinel: popstate → / (no user input recorded this page)");
  });

  it("preserves unknown destinations/mechanisms verbatim (never invents)", () => {
    expect(
      formatNavSentinelLine({
        mechanism: "navigation-api:unknown",
        destination: "(destination not reported)",
        sinceInputMs: 0,
      }),
    ).toBe(
      "[arbx] nav-sentinel: navigation-api:unknown → (destination not reported) (0ms since last user input)",
    );
  });
});

describe("getNavigationApi — defensive probe", () => {
  it("returns null when window.navigation is absent (non-Chromium / jsdom-less)", () => {
    expect(getNavigationApi({} as Window)).toBeNull();
  });

  it("returns the API object only when addEventListener is a function", () => {
    const fake = { addEventListener: () => {}, removeEventListener: () => {} };
    expect(getNavigationApi({ navigation: fake } as unknown as Window)).toBe(fake);
    expect(
      getNavigationApi({ navigation: { nope: true } } as unknown as Window),
    ).toBeNull();
  });
});
