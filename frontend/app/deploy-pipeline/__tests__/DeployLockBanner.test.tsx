import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { DeployLockBanner } from "../DeployLockBanner";
import { DEPLOY_LOCK } from "../data";

/**
 * FE-0059 (§64) — the deploy console reflects the REAL workflow: DEPLOY LOCKED
 * under the operator protocol, with the lock reasons and their unlock
 * conditions. These pin:
 *  - the LOCKED state renders verbatim with protocol grounding;
 *  - every reason (checklist/tests/review/regression/CI/operator) is present
 *    with its unlock condition and source anchor;
 *  - the nivel-(b) live-status declaration (RULE 00: no fabricated PASS/FAIL);
 *  - R1 determinism.
 */
describe("DeployLockBanner — §64 deploy locked with reasons", () => {
  const html = renderToStaticMarkup(<DeployLockBanner />);

  it("renders DEPLOY LOCKED verbatim with the protocol since-date", () => {
    expect(html).toContain("DEPLOY LOCKED");
    expect(html).toContain(`desde ${DEPLOY_LOCK.since}`);
    expect(html).toContain("PROTOCOLO ABSOLUTO");
  });

  it("every §64 reason category is present with unlock condition + source", () => {
    for (const id of ["checklist", "tests", "review", "regression", "ci", "operator"]) {
      expect(html).toContain(`data-testid="deploy-lock-reason-${id}"`);
    }
    for (const r of DEPLOY_LOCK.reasons) {
      expect(html).toContain(r.label);
      expect(html).toContain(r.unlock_condition);
      expect(html).toContain(r.source);
    }
    expect(html).toContain("desbloquea cuando:");
  });

  it("declares nivel-(b) live status — never fabricates a per-reason PASS/FAIL", () => {
    expect(html).toContain('data-testid="deploy-lock-live-status"');
    expect(html).toContain("nivel-(b)");
    expect(html).toContain("NO se muestra ni se fabrica");
    // no fabricated status badges on the reasons
    expect(html).not.toMatch(/deploy-lock-reason-[a-z]+[^>]*>.*(PASS|FAIL)/s);
  });

  it("grounds the lock on verifiable artifacts (KNOWN_GOOD_REVISION gate referenced)", () => {
    expect(html).toContain("KNOWN_GOOD_REVISION");
    expect(html).toContain("scripts/deploy.sh");
  });

  it("R1: SSR render is deterministic", () => {
    expect(renderToStaticMarkup(<DeployLockBanner />)).toBe(html);
  });
});
