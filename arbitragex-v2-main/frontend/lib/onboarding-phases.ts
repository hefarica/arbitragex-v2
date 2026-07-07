/**
 * Single source of truth for onboarding phase metadata.
 *
 * Used by:
 *   - `app/onboarding/page.tsx` (the index)
 *   - `app/onboarding/{n}-{slug}/page.tsx` (each phase landing)
 *   - `features/onboarding/OnboardingPhaseStub.tsx` (the shared stub UI)
 *
 * NOTE on `endpoint` and `exampleBody`: these describe the admission contract
 * the api-server is expected to implement for the matching phase. Phase 1 is
 * already live (`/admin/onboarding/1/complete`); 2–5 are documented in
 * `docs/governance/DATA-MATRIX.md` and may not all be wired yet. The stub UI
 * is honest about that — it never claims values exist that don't.
 */

export type PhaseNumber = 1 | 2 | 3 | 4 | 5;

export type PhaseMeta = {
  n: PhaseNumber;
  slug: string;
  title: string;
  blurb: string;
  solicits: string[];
  endpoint: string;
  /**
   * Illustrative request body shape. Values are placeholders the operator
   * must replace from their own secret manager — we never invent values.
   */
  exampleBody: Record<string, unknown>;
};

export const PHASES: PhaseMeta[] = [
  {
    n: 1,
    slug: "1-init",
    title: "Install & boot",
    blurb:
      "Generate infra tokens (admin, edge, JWT), unseal Vault, set the Grafana admin password. The stack must reach healthy /status before phase 2 unlocks.",
    solicits: [
      "ARBX_ADMIN_TOKEN",
      "ARBX_EDGE_TOKEN",
      "JWT_SECRET",
      "GRAFANA_ADMIN_PASSWORD",
      "vault unseal shares",
    ],
    endpoint: "/admin/onboarding/1/complete",
    exampleBody: {
      confirmed_by: "<operator-handle>",
      vault_sealed_healthy: true,
      notes: "<optional context>",
    },
  },
  {
    n: 2,
    slug: "2-connect",
    title: "Connect primary services",
    blurb:
      "Attach real RPCs per enabled chain so searcher-rs stops being idle. Optionally add a Slack webhook for warning-level alerts.",
    solicits: [
      "RPC_HTTP_<chain_id>",
      "RPC_WS_<chain_id>",
      "SLACK_WEBHOOK_URL (optional)",
    ],
    endpoint: "/admin/onboarding/2/complete",
    exampleBody: {
      confirmed_by: "<operator-handle>",
      rpcs: [
        { chain_id: 1, http: "<RPC_HTTP_1>", ws: "<RPC_WS_1>" },
      ],
      slack_webhook: "<SLACK_WEBHOOK_URL or null>",
      rpc_probe_ok: true,
    },
  },
  {
    n: 3,
    slug: "3-advanced",
    title: "Advanced features",
    blurb:
      "Choose a simulation provider, a token-safety provider, sign off on the scoring weights, import any initial blacklist.",
    solicits: [
      "ANVIL_FORK_URL or Tenderly creds",
      "token-safety provider + base URL + API key",
      "scoring_weights set + sign-off",
      "blacklist seed (optional)",
    ],
    endpoint: "/admin/onboarding/3/complete",
    exampleBody: {
      confirmed_by: "<operator-handle>",
      sim_provider: "<anvil|tenderly>",
      sim_endpoint: "<SIM_PROVIDER_URL>",
      token_safety: { provider: "<name>", base_url: "<url>", api_key_ref: "<vault-path>" },
      scoring_weights_signed_off: true,
    },
  },
  {
    n: 4,
    slug: "4-testing",
    title: "Real testing (paper-mode)",
    blurb:
      "Populate the relay catalog (DB, not code), attach a zero-balance Flashbots signer, deploy the Cloudflare Tunnel for admin access.",
    solicits: [
      "relay catalog entries (DB rows)",
      "FLASHBOTS_SIGNER_KEY (verified zero balance)",
      "CF_API_TOKEN + domain + tunnel token",
    ],
    endpoint: "/admin/onboarding/4/complete",
    exampleBody: {
      confirmed_by: "<operator-handle>",
      relay_count: 0,
      signer_zero_balance_verified: true,
      cf_tunnel_deployed: true,
    },
  },
  {
    n: 5,
    slug: "5-production",
    title: "Production operation",
    blurb:
      "Flip paper-mode off, wire PagerDuty, wire off-site backups, record incident contacts, confirm a capital cap signed by the operator.",
    solicits: [
      "PAGERDUTY_INTEGRATION_KEY",
      "B2 creds + age pubkey",
      "incident contacts (name + phone + email)",
      "capital cap (USD) + sign-off",
    ],
    endpoint: "/admin/onboarding/5/complete",
    exampleBody: {
      confirmed_by: "<operator-handle>",
      pagerduty_integration_key_ref: "<vault-path>",
      backup_destination: "b2://<bucket>",
      capital_cap_usd: 0,
      incident_contacts_count: 0,
      paper_mode_off: true,
    },
  },
];

export function getPhase(n: PhaseNumber): PhaseMeta {
  const p = PHASES.find((x) => x.n === n);
  if (!p) throw new Error(`unknown phase ${n}`);
  return p;
}
