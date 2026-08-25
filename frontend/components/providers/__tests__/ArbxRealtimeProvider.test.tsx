// frontend/components/providers/__tests__/ArbxRealtimeProvider.test.tsx
//
// FE-MASTER · FE-0008 — SSR smoke: the provider is a NULL RENDER (all socket
// and timer work lives in useEffect, which never runs under
// renderToStaticMarkup). What must hold on the server: children pass through
// untouched and NOTHING realtime leaks into the markup — R1: hydration is
// byte-identical because the first client render is the same null render.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { ArbxRealtimeProvider } from "../ArbxRealtimeProvider";

describe("ArbxRealtimeProvider — SSR smoke (R1 null render)", () => {
  it("passes children through and renders nothing of its own", () => {
    const html = renderToStaticMarkup(
      React.createElement(
        ArbxRealtimeProvider,
        null,
        React.createElement("div", { "data-testid": "child" }, "content"),
      ),
    );
    expect(html).toBe('<div data-testid="child">content</div>');
  });

  it("renders as an empty fragment with no children (root-layout mount form)", () => {
    const html = renderToStaticMarkup(React.createElement(ArbxRealtimeProvider, null));
    expect(html).toBe("");
  });
});
