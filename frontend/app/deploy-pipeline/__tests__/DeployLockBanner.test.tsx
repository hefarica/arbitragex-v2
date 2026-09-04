import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { DeployLockBanner } from "../DeployLockBanner";
import { DEPLOY_LOCK } from "../data";

/**
 * FE-0059 (§64) → DAPP-DEPLOYLOCK-TRUTH (2026-09-04): the 2026-08-23 protocol
 * ran its terminal WP-F gate on 2026-08-25 (PR #464 → main cfafa012, deploy
 * verified) and since 2026-08-29 the operative protocol is the operator's
 * PR-per-anomaly flow. These tests pin that the console NEVER regresses to the
 * stale "DEPLOY LOCKED" claim while the grounded facts hold:
 *  - the resolved state renders with protocol + resolution anchors;
 *  - every original reason (checklist/tests/review/regression/CI/operator) is
 *    present with its unlock condition, source anchor, a status badge and
 *    DATED static evidence (no live claim — nivel-(b), RULE 00);
 *  - the operator-only remainder (WP-F-style authorization, D10, S4 flips) is
 *    rendered as PENDIENTE OPERADOR, never self-approved;
 *  - R1 determinism.
 */
describe("DeployLockBanner — DAPP-DEPLOYLOCK-TRUTH resolved state", () => {
  const html = renderToStaticMarkup(<DeployLockBanner />);

  it("renders the executed-gate state with protocol + resolution dates", () => {
    expect(html).toContain("PROTOCOLO DEPLOY — GATE FINAL EJECUTADO");
    expect(html).toContain(`protocolo desde ${DEPLOY_LOCK.since}`);
    expect(html).toContain(`resuelto ${DEPLOY_LOCK.resolved_at}`);
    expect(html).toContain("PROTOCOLO ABSOLUTO");
    expect(html).toContain("#470..#518");
  });

  it("never renders the stale lock claim while the protocol is resolved", () => {
    expect(html).not.toContain("DEPLOY LOCKED");
  });

  it("every §64 reason category is present with unlock condition + source + dated evidence", () => {
    for (const id of ["checklist", "tests", "review", "regression", "ci", "operator"]) {
      expect(html).toContain(`data-testid="deploy-lock-reason-${id}"`);
    }
    for (const r of DEPLOY_LOCK.reasons) {
      expect(html).toContain(r.label);
      expect(html).toContain(r.unlock_condition);
      expect(html).toContain(r.source);
      expect(html).toContain(`data-status="${r.status}"`);
      expect(html).toContain(r.status_evidence);
      expect(html).toContain(r.status_date);
    }
    expect(html).toContain("desbloquea cuando:");
    expect(html).toContain("evidencia (");
  });

  it("statuses are the honest split: 4 satisfied, 1 superseded, 1 operator-pending", () => {
    const by = (s: string) => DEPLOY_LOCK.reasons.filter((r) => r.status === s);
    expect(by("satisfied")).toHaveLength(4);
    expect(by("superseded").map((r) => r.id)).toEqual(["ci"]);
    expect(by("operator-pending").map((r) => r.id)).toEqual(["operator"]);
    expect(html).toContain("PENDIENTE OPERADOR");
  });

  it("declares nivel-(b) — static dated evidence, never a fabricated live status", () => {
    expect(html).toContain('data-testid="deploy-lock-live-status"');
    expect(html).toContain("nivel-(b)");
    expect(html).toContain("NO estado vivo ni fabricado");
  });

  it("still grounds the operator authorization on verifiable artifacts", () => {
    expect(html).toContain("KNOWN_GOOD_REVISION");
    expect(html).toContain("scripts/deploy.sh");
    expect(html).toContain("operador-only");
  });

  it("R1: SSR render is deterministic", () => {
    expect(renderToStaticMarkup(<DeployLockBanner />)).toBe(html);
  });
});
