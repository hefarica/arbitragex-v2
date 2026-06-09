"use client";

/**
 * WalletIntentPanel — the safe transaction-intent pipeline.
 *
 * The operator can DESCRIBE a transaction they would make and run it through:
 *   1. Simulate (mandatory pre-flight — fail-honest if no runtime is wired)
 *   2. Register intent → ALWAYS terminates at BROADCAST_DISABLED
 *
 * The "Broadcast / Execute" affordance exists ONLY as a permanently-disabled
 * shell with an explicit reason. There is no enabled send path anywhere here.
 *
 * SSR-safe: no wallet-derived live value is read at first paint; the form is
 * static and the results render only after the user acts.
 */

import { useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useWalletIntent } from "@/hooks/useWalletIntent";
import { useTransactionSimulation } from "@/hooks/useTransactionSimulation";

export function WalletIntentPanel() {
  const [kind, setKind] = useState("swap");
  const [to, setTo] = useState("");
  const [note, setNote] = useState("");

  const { result: intent, error: intentError, submitting, submitIntent } = useWalletIntent();
  const { result: sim, error: simError, running, simulate } = useTransactionSimulation();

  const draft = { kind, to: to || undefined, note: note || undefined };

  return (
    <Card data-testid="wallet-intent-panel">
      <CardHeader>
        <CardTitle className="flex items-center justify-between">
          <span>Transaction intent (read-only)</span>
          <Badge variant="success">Broadcast disabled</Badge>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-5">
        <p className="text-sm text-muted-foreground">
          Describe a transaction to record an INTENT for review. Nothing is signed or sent. Every
          intent terminates at <code className="font-mono">BROADCAST_DISABLED</code>.
        </p>

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
          <div className="space-y-1.5">
            <Label htmlFor="intent-kind">Kind</Label>
            <Input id="intent-kind" value={kind} onChange={(e) => setKind(e.target.value)} placeholder="swap" />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="intent-to">To (review only)</Label>
            <Input id="intent-to" value={to} onChange={(e) => setTo(e.target.value)} placeholder="0x…" />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="intent-note">Note</Label>
            <Input id="intent-note" value={note} onChange={(e) => setNote(e.target.value)} placeholder="optional" />
          </div>
        </div>

        <div className="flex flex-wrap gap-3">
          <Button type="button" variant="outline" disabled={running} onClick={() => simulate(draft)} data-testid="intent-simulate">
            {running ? "Simulating…" : "Simulate (required)"}
          </Button>
          <Button type="button" variant="secondary" disabled={submitting} onClick={() => submitIntent(draft)} data-testid="intent-register">
            {submitting ? "Recording…" : "Register intent"}
          </Button>
          {/* The broadcast/execute affordance is a PERMANENT disabled shell. */}
          <Button
            type="button"
            variant="destructive"
            disabled
            aria-disabled="true"
            title="broadcast_disabled_by_policy"
            data-testid="intent-broadcast-disabled"
          >
            Broadcast disabled by policy
          </Button>
        </div>

        {/* Simulation result */}
        {(sim || simError) && (
          <div className="rounded-md border p-3 text-sm" data-testid="simulation-result">
            <p className="font-medium">Simulation</p>
            {simError ? (
              <p className="text-muted-foreground">error: {simError}</p>
            ) : sim ? (
              <div className="mt-1 space-y-1">
                <p>
                  state: <code className="font-mono">{sim.state}</code> · simulated:{" "}
                  <code className="font-mono">{String(sim.simulated)}</code>
                </p>
                <p className="text-muted-foreground">{sim.message}</p>
                <div className="flex flex-wrap gap-2">
                  <Badge variant="success">broadcast_allowed: false</Badge>
                  <Badge variant="outline">{sim.broadcast_reason}</Badge>
                </div>
              </div>
            ) : null}
          </div>
        )}

        {/* Intent result */}
        {(intent || intentError) && (
          <div className="rounded-md border p-3 text-sm" data-testid="intent-result">
            <p className="font-medium">Intent</p>
            {intentError ? (
              <p className="text-muted-foreground">error: {intentError}</p>
            ) : intent ? (
              <div className="mt-1 space-y-1">
                <p>
                  id: <code className="font-mono">{intent.intent_id}</code>
                </p>
                <p>
                  terminal state: <code className="font-mono" data-testid="intent-terminal-state">{intent.state}</code>
                </p>
                <p className="text-muted-foreground">lifecycle: {intent.lifecycle.join(" → ")}</p>
                <div className="flex flex-wrap gap-2">
                  <Badge variant="success">broadcast_allowed: false</Badge>
                  <Badge variant="outline">{intent.reason}</Badge>
                  <Badge variant="success">capital_exposed: 0</Badge>
                </div>
                <p className="text-muted-foreground">{intent.message}</p>
              </div>
            ) : null}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
