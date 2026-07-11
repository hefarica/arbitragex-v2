"use client";

/**
 * TransactionIntentPreview — renders a legible EIP-712 TransactionIntent (never opaque calldata)
 * alongside the Policy Engine's per-gate verdict. The sign affordance is a DISABLED shell: it is
 * gated by the policy result (deny-by-default) and, in the current posture, is always blocked
 * because live/simulation/readiness gates are closed. This component performs NO signing and holds
 * no key — it is a preview + policy display only. The actual signTypedData wiring is a separate,
 * future, gated increment.
 */

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { legibleIntentPreview, type TransactionIntent } from "@/lib/web3/intent";
import { evaluatePolicy, type PolicyContext } from "@/lib/web3/policy";

export function TransactionIntentPreview({ intent, ctx }: { intent: TransactionIntent; ctx: PolicyContext }) {
  const rows = legibleIntentPreview(intent);
  const policy = evaluatePolicy(intent, ctx);
  const passCount = policy.gates.filter((g) => g.status === "pass").length;
  const signBlocked = !policy.allow;

  return (
    <Card data-testid="transaction-intent-preview">
      <CardHeader>
        <CardTitle>Transaction intent (EIP-712 — legible)</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <p className="text-xs text-muted-foreground/70">
          You sign this structured intent, never opaque calldata. The calldata hash binds it to the exact
          simulated transaction; the executor proceeds only if the calldata matches.
        </p>

        <dl className="grid grid-cols-1 gap-2 sm:grid-cols-2" data-testid="intent-rows">
          {rows.map((r) => (
            <div key={r.label}>
              <dt className="text-xs uppercase tracking-widest text-muted-foreground/70">{r.label}</dt>
              <dd className="mt-1 break-all font-mono text-xs">{r.value}</dd>
            </div>
          ))}
        </dl>

        <div>
          <div className="mb-2 text-xs uppercase tracking-widest text-muted-foreground/70">
            Policy gates ({passCount}/{policy.gates.length})
          </div>
          <div className="flex flex-wrap gap-1" data-testid="policy-gates">
            {policy.gates.map((g) => (
              <Badge
                key={g.name}
                variant={g.status === "pass" ? "success" : "destructive"}
                title={g.reason ?? "pass"}
                data-testid={`gate-${g.name}`}
              >
                {g.name}
              </Badge>
            ))}
          </div>
        </div>

        {/* Permanent disabled shell — mirrors the existing broadcast shell. No signer is ever invoked. */}
        <button
          type="button"
          disabled
          aria-disabled="true"
          data-testid="intent-sign-button"
          title={signBlocked ? "blocked_by_policy" : "sign_disabled"}
          className="cursor-not-allowed rounded-md border px-3 py-2 text-sm opacity-50"
        >
          {signBlocked ? `Sign blocked — ${policy.denied.length} gate(s) failing` : "Sign intent (disabled)"}
        </button>
      </CardContent>
    </Card>
  );
}
