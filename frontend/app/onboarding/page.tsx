import Link from "next/link";
import { AlertCircleIcon, CheckCircle2Icon, CircleIcon, LockIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { PageHeader } from "@/components/page-header";
import { getOnboardingStatus, type OnboardingStatus } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

type Phase = {
  n: 1 | 2 | 3 | 4 | 5;
  slug: string;
  title: string;
  blurb: string;
  solicits: string[];
};

const PHASES: Phase[] = [
  {
    n: 1,
    slug: "/onboarding/1-init",
    title: "Install & boot",
    blurb:
      "Generate infra tokens (admin, edge, JWT), unseal Vault, set the Grafana admin password. The stack must reach healthy /status before phase 2 unlocks.",
    solicits: ["ARBX_ADMIN_TOKEN", "ARBX_EDGE_TOKEN", "JWT_SECRET", "GRAFANA_ADMIN_PASSWORD", "vault unseal shares"],
  },
  {
    n: 2,
    slug: "/onboarding/2-connect",
    title: "Connect primary services",
    blurb:
      "Attach real RPCs per enabled chain so searcher-rs stops being idle. Optionally add a Slack webhook for warning-level alerts.",
    solicits: ["RPC_HTTP_1 + RPC_WS_1 (per chain)", "SLACK_WEBHOOK_URL (optional)"],
  },
  {
    n: 3,
    slug: "/onboarding/3-advanced",
    title: "Advanced features",
    blurb:
      "Choose a simulation provider, a token-safety provider, sign off on the scoring weights, import any initial blacklist.",
    solicits: ["ANVIL_FORK_URL or Tenderly", "token-safety provider + base URL + API key", "scoring_weights set + sign-off"],
  },
  {
    n: 4,
    slug: "/onboarding/4-testing",
    title: "Real testing (paper-mode)",
    blurb:
      "Populate the relay catalog (DB, not code), attach a zero-balance Flashbots signer, deploy the Cloudflare Tunnel for admin access.",
    solicits: ["relay catalog entries", "FLASHBOTS_SIGNER_KEY (zero balance)", "CF_API_TOKEN + domain + tunnel token"],
  },
  {
    n: 5,
    slug: "/onboarding/5-production",
    title: "Production operation",
    blurb:
      "Flip paper-mode off, wire PagerDuty, wire off-site backups, record incident contacts, confirm a capital cap signed by the operator.",
    solicits: ["PAGERDUTY_INTEGRATION_KEY", "B2 creds + age pubkey", "incident contacts", "capital cap sign-off"],
  },
];

type PhaseStatus = "done" | "locked" | "next";

function phaseStatus(s: OnboardingStatus, n: number): PhaseStatus {
  const doneAt = [
    s.phase_1_completed_at,
    s.phase_2_completed_at,
    s.phase_3_completed_at,
    s.phase_4_completed_at,
    s.phase_5_completed_at,
  ];
  if (doneAt[n - 1]) return "done";
  // "next" is the first non-done; everything after is locked.
  const firstUndone = doneAt.findIndex((v) => !v);
  return n - 1 === firstUndone ? "next" : "locked";
}

export default async function OnboardingIndexPage() {
  const res = await getOnboardingStatus();

  if (!res.ok) {
    return (
      <>
        <PageHeader
          title="Onboarding"
          lede="Progressive setup in five phases. The app refuses to operate features whose phase isn't complete."
        />
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>edge error</AlertTitle>
          <AlertDescription>
            Could not read onboarding progress: <code className="font-mono text-xs">{res.error}</code>
            <p className="mt-2 text-sm">
              This usually means the DB is not reachable or migration 018 hasn't been applied.
            </p>
          </AlertDescription>
        </Alert>
      </>
    );
  }

  const s = res.data;

  return (
    <>
      <PageHeader
        title="Onboarding"
        lede="Progressive setup in five phases. Each phase unlocks a specific set of features. Phase N cannot complete while phase N−1 is still pending — the app never pretends to operate on missing data."
        meta={[
          `org: ${s.org_id}`,
          `last updated ${new Date(s.updated_at).toLocaleString()}`,
        ]}
      />

      <div className="flex flex-col gap-4">
        {PHASES.map((p) => {
          const st = phaseStatus(s, p.n);
          const Icon = st === "done" ? CheckCircle2Icon : st === "locked" ? LockIcon : CircleIcon;
          const cardClass =
            st === "done"
              ? "border-success/40"
              : st === "next"
                ? "border-primary/50 ring-1 ring-primary/20"
                : "opacity-70";
          return (
            <Card key={p.n} className={cardClass}>
              <CardHeader>
                <div className="flex items-start justify-between gap-4">
                  <div className="flex items-start gap-3">
                    <Icon
                      className={
                        st === "done"
                          ? "mt-1 size-5 shrink-0 text-success"
                          : st === "next"
                            ? "mt-1 size-5 shrink-0 text-primary"
                            : "mt-1 size-5 shrink-0 text-muted-foreground"
                      }
                    />
                    <div>
                      <CardTitle className="text-base">
                        Phase {p.n} — {p.title}
                      </CardTitle>
                      <CardDescription className="mt-1">{p.blurb}</CardDescription>
                    </div>
                  </div>
                  <Badge
                    variant={st === "done" ? "success" : st === "next" ? "default" : "outline"}
                    className="shrink-0"
                  >
                    {st === "done" ? "complete" : st === "next" ? "next" : "locked"}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent className="flex flex-col gap-3">
                <div className="text-xs uppercase tracking-widest text-muted-foreground">
                  Solicits
                </div>
                <ul className="flex flex-wrap gap-2 text-xs">
                  {p.solicits.map((x) => (
                    <li key={x}>
                      <code className="rounded bg-muted px-2 py-0.5 font-mono">{x}</code>
                    </li>
                  ))}
                </ul>
                {st === "done" && p.n === 1 && (
                  <p className="text-xs text-muted-foreground">
                    Completed by <code>{s.phase_1_completed_by}</code> at{" "}
                    {s.phase_1_completed_at && new Date(s.phase_1_completed_at).toLocaleString()}
                    . Vault healthy: {s.phase_1_vault_sealed_healthy ? "yes" : "no"}.
                  </p>
                )}
                {st === "next" && p.n === 1 ? (
                  <Link
                    href="/onboarding/1-init"
                    className="inline-flex w-max items-center gap-2 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90"
                  >
                    Start phase 1 →
                  </Link>
                ) : st === "next" ? (
                  <p className="text-xs text-muted-foreground">
                    This phase's UI form ships with a later PR. Today it can be completed by
                    calling the admin endpoint directly (see{" "}
                    <code>docs/governance/DATA-MATRIX.md</code>).
                  </p>
                ) : null}
              </CardContent>
            </Card>
          );
        })}
      </div>
    </>
  );
}
