"use client";

/**
 * GSimSmokeTestCard — Interactive simulator-v2 readiness gate card.
 *
 * Displays G-SIM-1 status and provides a "Run Sepolia Smoke Test" button
 * that validates the REVM multi-step simulation path end-to-end.
 * Lives inside the /live-readiness page grid alongside other gate cards.
 */

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Loader2, Play, CheckCircle2, XCircle, Activity } from "lucide-react";

interface SmokeTestResult {
  passed: boolean;
  gas_used_total: number;
  gas_price_wei: string;
  simulated_profit_token_in: string;
  fail_reason?: string;
  wrapped_calldata?: string;
}

const SEPOLIA_WETH = "0xfff9976782d46cc05630d1f6ebab18b2324d6b14";
const SEPOLIA_USDC = "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238";

export function GSimSmokeTestCard() {
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<SmokeTestResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [enabled, setEnabled] = useState(false);

  async function runSmokeTest() {
    setRunning(true);
    setError(null);
    setResult(null);

    try {
      const candidate = {
        opportunity_id: crypto.randomUUID(),
        chain_id: 11155111,
        block_number: 0,
        route_fingerprint: "sepolia_smoke_weth_usdc_v2",
        pool_addresses: ["0x0000000000000000000000000000000000000000"],
        token_addresses: [SEPOLIA_WETH, SEPOLIA_USDC],
        dex_adapters: ["uniswap_v2", "uniswap_v2"],
        amount_in: "1000000000000000000",
        expected_amount_out: "0",
        gross_profit: "0",
        decimals: {
          [SEPOLIA_WETH.toLowerCase()]: 18,
          [SEPOLIA_USDC.toLowerCase()]: 6,
        },
      };

      const res = await fetch("/api/sim-ctl/simulate", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          route_source: "simctl_lookup",
          candidate,
        }),
      });

      const body = (await res.json()) as SmokeTestResult & { error?: string; detail?: string };

      if (!res.ok) {
        setError(body.detail || body.error || `HTTP ${res.status}`);
        return;
      }

      setResult(body);
      if (body.passed) {
        setEnabled(true);
      }
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  }

  async function enableSimulatorV2() {
    try {
      const res = await fetch("/api/admin/simulator-v2-ready", {
        method: "POST",
        headers: { "content-type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ ready: true }),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setEnabled(true);
    } catch (e) {
      setError(`Enable failed: ${(e as Error).message}`);
    }
  }

  const status = result
    ? result.passed
      ? "green"
      : "red"
    : "red";

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center justify-between text-base">
          <span className="flex items-center gap-2">
            <Activity className="size-4 text-muted-foreground" />
            G-SIM-1 (Simulator V2)
          </span>
          <Badge
            variant={
              status === "green" ? "success" : "destructive"
            }
          >
            {status === "green" ? "PASS" : "RED"}
          </Badge>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4 pt-0">
        <p className="text-sm text-muted-foreground">
          Validates the REVM multi-step simulation path against Sepolia contracts.
          A passing smoke test enables the simulator-v2 readiness gate.
        </p>

        <div className="flex flex-wrap gap-2">
          <Button
            size="sm"
            onClick={runSmokeTest}
            disabled={running}
            data-testid="run-smoke-test"
          >
            {running ? (
              <Loader2 className="size-4 mr-1 animate-spin" />
            ) : (
              <Play className="size-4 mr-1" />
            )}
            Run Sepolia Smoke Test
          </Button>

          {enabled && (
            <Button
              size="sm"
              variant="outline"
              onClick={enableSimulatorV2}
              disabled={enabled}
              data-testid="enable-simulator-v2"
            >
              <CheckCircle2 className="size-4 mr-1" />
              {enabled ? "Enabled" : "Enable SimulatorV2"}
            </Button>
          )}
        </div>

        {error && (
          <div className="rounded-md bg-destructive/10 p-3 text-sm text-destructive flex items-start gap-2">
            <XCircle className="size-4 mt-0.5 shrink-0" />
            {error}
          </div>
        )}

        {result && (
          <div className="space-y-2 text-sm">
            <div className="flex items-center gap-2">
              {result.passed ? (
                <CheckCircle2 className="size-4 text-green-500" />
              ) : (
                <XCircle className="size-4 text-destructive" />
              )}
              <span className={result.passed ? "text-green-500" : "text-destructive"}>
                {result.passed ? "SIM_SUCCESS" : `Failed: ${result.fail_reason || "unknown"}`}
              </span>
            </div>
            {result.passed && (
              <>
                <div className="grid grid-cols-2 gap-2 text-muted-foreground">
                  <div>Gas used: {result.gas_used_total}</div>
                  <div>Profit: {result.simulated_profit_token_in}</div>
                </div>
                {result.wrapped_calldata && (
                  <div className="font-mono text-xs bg-muted p-2 rounded truncate">
                    Calldata: {result.wrapped_calldata.slice(0, 50)}...
                  </div>
                )}
              </>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
