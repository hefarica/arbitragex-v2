/**
 * Canonical route catalog for the QuantumX / ArbitrageX v2 operator console.
 *
 * Derived from `frontend/app/**\/page.tsx` (44 routes at time of authoring).
 * Each route is tagged so the live functional suite can apply the right
 * expectations:
 *
 *   - group:   sidebar grouping (observe | control | setup | admin | flow | misc)
 *   - admin:   true  -> page calls an admin-gated endpoint; an unauthenticated
 *                       load is EXPECTED to surface a 401 on a backing fetch
 *                       (the page must handle it gracefully, never fabricate).
 *   - form:    true  -> page has an interactive form whose client-side
 *                       validation we assert (no submit performed read-only).
 *
 * Doctrine: read-only. No route here triggers capital, signing, or broadcast.
 */

export interface RouteSpec {
  path: string;
  /** Human label for report rows. */
  label: string;
  group: "observe" | "control" | "setup" | "admin" | "flow" | "misc";
  /** Page is expected to hit an admin-gated endpoint when loaded unauth. */
  admin?: boolean;
  /** Page renders an interactive form we can validate client-side. */
  form?: boolean;
}

export const ROUTES: RouteSpec[] = [
  // --- Observe -----------------------------------------------------------
  { path: "/", label: "Home / control plane", group: "observe" },
  { path: "/status", label: "System status", group: "observe" },
  { path: "/opportunities", label: "Convergence signals", group: "observe" },
  { path: "/executions", label: "Resolutions", group: "observe" },
  { path: "/risk", label: "Entropy & alerts", group: "observe" },
  { path: "/recon", label: "Recon & yield", group: "observe" },
  { path: "/operations", label: "Convergence metrics", group: "observe" },
  { path: "/route-outcomes", label: "Route outcomes", group: "observe" },
  { path: "/sed", label: "SED Pipeline", group: "observe" },

  // --- Control -----------------------------------------------------------
  { path: "/config", label: "Current config", group: "control" },
  { path: "/config/trading", label: "Trading config", group: "control", form: true },
  { path: "/strategies", label: "Resolution engines", group: "control" },
  { path: "/strategies/forge", label: "Strategy Forge", group: "control", form: true },
  { path: "/killswitch", label: "Kill-switch", group: "control", admin: true, form: true },
  { path: "/audit-logs", label: "Audit logs", group: "control" },
  { path: "/live-readiness", label: "Live readiness", group: "control" },

  // --- Setup -------------------------------------------------------------
  { path: "/wallets", label: "Observers", group: "setup" },
  { path: "/dex-registry", label: "Exchange registry", group: "setup" },
  { path: "/chains", label: "Chains", group: "setup" },
  { path: "/rpcs", label: "RPCs", group: "setup" },
  { path: "/pools", label: "Pools", group: "setup" },
  { path: "/settings", label: "Settings", group: "setup", form: true },
  { path: "/settings/credentials", label: "Credentials", group: "setup", form: true },

  // --- Admin (gated) -----------------------------------------------------
  { path: "/admin/topology", label: "Topology Vault", group: "admin", admin: true },
  { path: "/admin/chains", label: "Admin chains", group: "admin", admin: true },
  { path: "/admin/signin", label: "Admin sign-in", group: "admin", form: true },

  // --- Onboarding flow ---------------------------------------------------
  { path: "/onboarding", label: "Onboarding index", group: "flow" },
  { path: "/onboarding/1-init", label: "Onboarding 1 — init", group: "flow", form: true },
  { path: "/onboarding/2-connect", label: "Onboarding 2 — connect", group: "flow", form: true },
  { path: "/onboarding/3-advanced", label: "Onboarding 3 — advanced", group: "flow", form: true },
  { path: "/onboarding/4-testing", label: "Onboarding 4 — testing", group: "flow" },
  { path: "/onboarding/5-production", label: "Onboarding 5 — production", group: "flow" },

  // --- OMEGA-S5 ----------------------------------------------------------
  { path: "/omega-s5/core", label: "OMEGA-S5 core", group: "misc" },
  { path: "/omega-s5/factory", label: "OMEGA-S5 factory", group: "misc" },
  { path: "/omega-s5/adapters", label: "OMEGA-S5 adapters", group: "misc" },
  { path: "/omega-s5/crucible", label: "OMEGA-S5 crucible", group: "misc" },
  { path: "/omega-s5/operator", label: "OMEGA-S5 operator", group: "misc" },
  { path: "/omega-s5/drift", label: "OMEGA-S5 drift", group: "misc" },
  { path: "/omega-s5/registry", label: "OMEGA-S5 registry", group: "misc" },
  { path: "/omega-s5/wallets", label: "OMEGA-S5 wallets", group: "misc" },

  // --- Misc --------------------------------------------------------------
  { path: "/apex/allocator", label: "APEX allocator", group: "misc" },
  { path: "/deploy-pipeline", label: "Deploy pipeline", group: "misc" },
  { path: "/worker-health", label: "Worker health", group: "misc" },
];

/**
 * LEGACY page-level error banners only — the specific copy a page used to print
 * when it was *entirely* fail-closed (the old maroon full-page banner).
 *
 * Deliberately NARROW. We do NOT match a bare "edge error" or "zero trust":
 *  - a metric card honestly printing "edge error" for ONE failing upstream is a
 *    per-widget honest state, not a page failure (RULE 00 / R8);
 *  - "ZERO TRUST" appears as branding / doctrine copy in healthy pages.
 * Page-level error is now detected STRUCTURALLY via the EdgeState error surface
 * (see EDGE_STATE_ERROR_LABELS) scoped to <main>, not by scanning all body text.
 */
export const ERROR_BANNERS: RegExp[] = [
  /edge unreachable/i,
  /edge error:/i, // WITH colon — the old page-level banner prefix, not the inline card text
];

/**
 * EdgeState (frontend/components/EdgeState.tsx) BLOCKING variants render with
 * role="alert" and one of these uppercase status labels. Detecting THIS inside
 * <main> (and the absence of primary content) is what marks a route fail-closed.
 *
 * NO `\b` boundaries: textContent glues the label to the following title with no
 * whitespace (e.g. "EDGE ERRORPool registry unreachable"), so a trailing `\b`
 * would never match. We match the label as a plain substring via role + filter.
 */
export const EDGE_STATE_ERROR_LABELS = /EDGE ERROR|DISCONNECTED/;

/**
 * EdgeState NON-error variants (role="status"): honest empty / first-paint load.
 * These are NOT defects — a page showing one is rendered + honest, never FAIL.
 * Same no-`\b` rule (textContent is "NO DATANo RPCs registered yet…").
 */
export const EDGE_STATE_EMPTY_LOADING = /NO DATA|SYNCING/;

/**
 * RULE 00 (zero-mocks) — UNAMBIGUOUS fabrication markers in rendered body text.
 *
 * Deliberately limited to "lorem ipsum". Bare "mock"/"fake"/"dummy" are NOT
 * matched: they appear in legitimate anti-mock doctrine copy (e.g. the
 * /live-readiness check "zero mocks doctrine: no fabricated data", or "No
 * fabricated pool data is shown"), which is the OPPOSITE of a violation. Real
 * zero-mocks enforcement lives in honest-state-live.spec.ts (count===items.length,
 * no synthesized rows) and the no-hardcode lint — not a blunt body-text scan.
 */
export const FORBIDDEN_WORDS = /lorem ipsum/i;

/**
 * Admin endpoints whose 401 under an unauthenticated load is EXPECTED and
 * must NOT count as a page error.
 */
export const EXPECTED_401_PATHS = [
  "/api/admin/",
];
