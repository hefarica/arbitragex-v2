/**
 * GoNoGoSignOffCard SSR-only static markup tests.
 *
 * Like GoNoGoPanel.test.tsx, useEffect doesn't fire under
 * renderToStaticMarkup. We assert the structural invariants visible at
 * initial server render:
 *   - "A.9 formal sign-off" title rendered
 *   - The disclaimer "no sign button here" rendered
 *   - The static-render loading state shows "Loading sign-off ledger"
 *   - There is NO sign-off button (GO / NO_GO / submit-signature)
 *   - There is NO flip-to-live control
 *   - The runbook uses PLACEHOLDERS, never a real host/token literal
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { GoNoGoSignOffCard } from "../GoNoGoSignOffCard";

const html = renderToStaticMarkup(<GoNoGoSignOffCard />);

describe("GoNoGoSignOffCard (SSR initial state)", () => {
  it("renders the A.9 formal sign-off title", () => {
    expect(html).toMatch(/A\.9 formal sign-off/i);
  });

  it("asserts the no-sign-button disclaimer on every render", () => {
    expect(html).toMatch(/no sign button here/i);
  });

  it("renders loading state initially", () => {
    expect(html).toMatch(/Loading sign-off ledger/i);
  });

  it("renders the read-only Regenerate ledger control", () => {
    expect(html).toMatch(/Regenerate ledger/i);
  });

  it("regression: never renders a sign-off POST control", () => {
    expect(html).not.toMatch(/>Sign GO</i);
    expect(html).not.toMatch(/>Sign NO-GO</i);
    expect(html).not.toMatch(/>Sign-off</i);
    expect(html).not.toMatch(/>Submit signature</i);
    expect(html).not.toMatch(/>Record decision</i);
  });

  it("regression: never renders a flip-to-live control", () => {
    expect(html).not.toMatch(/>Flip to live</i);
    expect(html).not.toMatch(/>Enable live</i);
    expect(html).not.toMatch(/>Activate live</i);
  });

  it("runbook documents the admin POST and stays placeholder-only", () => {
    // The runbook TEXT names the admin endpoint (documentation, not control).
    expect(html).toContain("/admin/go-no-go/sign-off");
    expect(html).toContain("x-arbx-admin-token");
    // renderToStaticMarkup escapes < > in text nodes — assert the ESCAPED
    // placeholders (&lt;VPS-IP&gt;) so the test asserts exactly what ships.
    expect(html).toContain("&lt;VPS-IP&gt;");
    expect(html).toContain("&lt;LEDGER_HASH&gt;");
    // No literal credential material anywhere: the only legal followers of
    // the header name are the env placeholder ($ARBX_ADMIN_TOKEN) or an
    // angle-bracket placeholder (&lt;…&gt;). A real token (alphanumeric
    // literal) must never appear.
    expect(html).not.toMatch(/x-arbx-admin-token:\s+[A-Za-z0-9]/);
  });
});
