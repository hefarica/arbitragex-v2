/**
 * B1.b — Static-markup tests for the Chains Admin UI.
 *
 * Same toolkit-alignment constraint as SystemGuardBanner.test.tsx: we use
 * `renderToStaticMarkup` (no jsdom) to validate the SSR-deterministic
 * output. Stateful behavior (probe latency, dialog open/close, refresh)
 * is covered by integration tests that run in the browser preview.
 *
 * What these tests guard:
 *   - R1: the SSR markup never reads document.cookie / window — confirmed
 *     by the absence of cookie sentinels in the initial output.
 *   - Fail-honest: empty snapshot renders "No chains" without fabricating.
 *   - Fail-honest: error snapshot surfaces the verbatim error string.
 *   - Status badge: enabled=null ⇒ "Unknown" (regression alarm).
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { ChainStatusBadge } from "../ChainStatusBadge";
import { ChainsAdminClient, type ChainsAdminSnapshot } from "../ChainsAdminClient";
import type { AdminChainRow } from "@/lib/schemas";

const row: AdminChainRow = {
  id: 1,
  chain_id: 1,
  name: "ethereum",
  rpc_http_url: "https://eth-mainnet.g.alchemy.com/v2/redactedkey",
  rpc_ws_url: null,
  native_currency: "ETH",
  block_time_ms: 12000,
  enabled: true,
  config_hash: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
  notes: null,
  created_at: "2026-05-13T12:00:00.000Z",
  updated_at: "2026-05-13T12:34:56.789Z",
  created_by: "operator",
  updated_by: "operator",
};

describe("ChainStatusBadge", () => {
  it("renders Active when enabled=true", () => {
    const html = renderToStaticMarkup(<ChainStatusBadge enabled={true} />);
    expect(html).toContain("Active");
  });

  it("renders Disabled when enabled=false", () => {
    const html = renderToStaticMarkup(<ChainStatusBadge enabled={false} />);
    expect(html).toContain("Disabled");
  });

  it("renders Unknown when enabled=null (R8 fail-honest)", () => {
    const html = renderToStaticMarkup(<ChainStatusBadge enabled={null} />);
    expect(html).toContain("Unknown");
    expect(html).not.toContain("Active");
    expect(html).not.toContain("Disabled");
  });
});

describe("ChainsAdminClient — SSR markup", () => {
  it("renders the Chains Admin header and add-chain button", () => {
    const snap: ChainsAdminSnapshot = { rows: [row], source: "server-snapshot" };
    const html = renderToStaticMarkup(<ChainsAdminClient initialSnapshot={snap} />);
    expect(html).toContain("Chains Admin");
    expect(html).toContain("Add chain");
    expect(html).toContain("Refresh");
  });

  it("renders the chain row name + chain_id + redacted RPC URL", () => {
    const snap: ChainsAdminSnapshot = { rows: [row], source: "server-snapshot" };
    const html = renderToStaticMarkup(<ChainsAdminClient initialSnapshot={snap} />);
    expect(html).toContain("ethereum");
    expect(html).toContain("id 1");
    // RPC URL must be redacted — never leak the API key
    expect(html).not.toContain("redactedkey");
    expect(html).toContain("eth-mainnet.g.alchemy.com");
  });

  it("renders empty state when no rows and source=server-snapshot", () => {
    const snap: ChainsAdminSnapshot = { rows: [], source: "server-snapshot" };
    const html = renderToStaticMarkup(<ChainsAdminClient initialSnapshot={snap} />);
    expect(html).toContain("No chains");
  });

  it("surfaces snapshot error verbatim (R8 fail-honest)", () => {
    const snap: ChainsAdminSnapshot = {
      rows: [],
      source: "server-fetch-failed",
      error: "HTTP 502: bad gateway",
    };
    const html = renderToStaticMarkup(<ChainsAdminClient initialSnapshot={snap} />);
    expect(html).toContain("HTTP 502: bad gateway");
    expect(html).toContain("Admin chains endpoint error");
  });

  it("does not fabricate rows when auth required (server-auth-required)", () => {
    const snap: ChainsAdminSnapshot = {
      rows: [],
      source: "server-auth-required",
      error: "HTTP 401 — client will retry with admin cookie",
    };
    const html = renderToStaticMarkup(<ChainsAdminClient initialSnapshot={snap} />);
    expect(html).toContain("HTTP 401");
    // No row markup expected
    expect(html).not.toContain("id 1");
  });

  it("filter tabs reflect counts deterministically across SSR", () => {
    const disabledRow: AdminChainRow = { ...row, chain_id: 137, name: "polygon", enabled: false };
    const snap: ChainsAdminSnapshot = { rows: [row, disabledRow], source: "server-snapshot" };
    const html = renderToStaticMarkup(<ChainsAdminClient initialSnapshot={snap} />);
    expect(html).toContain("All (2)");
    expect(html).toContain("Active (1)");
    expect(html).toContain("Disabled (1)");
  });

  it("does not read document.cookie or window during SSR (R1)", () => {
    const snap: ChainsAdminSnapshot = { rows: [row], source: "server-snapshot" };
    // Render under a sandbox-like environment: jsdom is not in scope for
    // this test runner, but we still spot-check that no cookie sentinel
    // leaks into the markup (admin token must never be inlined).
    const html = renderToStaticMarkup(<ChainsAdminClient initialSnapshot={snap} />);
    expect(html).not.toContain("__session_active__");
    // The KeyRound warning banner is gated on isMounted — its icon class
    // must NOT appear in the SSR markup. (The same string can show up as
    // the disabled "Add chain" button's `title=` tooltip — that is fine.)
    expect(html).not.toContain("lucide-key-round");
    // The fmtTimestamp call is also gated on isMounted; the cell shows
    // a placeholder ellipsis instead of the row's updated_at on SSR.
    expect(html).not.toContain("2026-05-13 12:34:56Z");
  });
});
