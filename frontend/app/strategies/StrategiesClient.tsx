/**
 * Sprint 2 Task 2.3 — Strategies client component (5-tab container).
 *
 * Receives initial trading_config + strategy_catalog snapshots from the
 * Server Component. Holds the "live" config in state so tab edits are
 * mirrored across tabs immediately on save. Each tab triggers
 * `putTradingConfig` itself; this component just wires them together.
 *
 * Admin token + actor are read from environment-style state passed in.
 * The admin httpOnly cookie carries auth via fetch credentials="include".
 */
"use client";

import { useEffect, useState } from "react";
import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getAdminToken } from "@/lib/admin-token";
import type { StrategyCatalogEntry, TradingConfigConfigured, TradingConfigResponse } from "@/lib/schemas";

import { CapitalRiskTab } from "./tabs/CapitalRiskTab";
import { StrategyCatalogTab } from "./tabs/StrategyCatalogTab";
import { DexesTab } from "./tabs/DexesTab";
import { PoolsTab } from "./tabs/PoolsTab";
import { MevRelaysTab } from "./tabs/MevRelaysTab";
import { TokenAllowlistTab } from "./tabs/TokenAllowlistTab";
import { AuditTab } from "./tabs/AuditTab";
import { SimulationTab } from "./tabs/SimulationTab";

interface Props {
  initialConfig: TradingConfigResponse | null;
  initialCatalog: StrategyCatalogEntry[];
  initialError: string | null;
}

export function StrategiesClient({ initialConfig, initialCatalog, initialError }: Props) {
  const [config, setConfig] = useState<TradingConfigConfigured | null>(
    initialConfig?.configured ? initialConfig : null,
  );
  // Admin token + actor are read from the operator's existing httpOnly session
  // (admin-token.ts wraps the cookie probe). Read in useEffect to keep the
  // first render deterministic across SSR/CSR (R1).
  const [adminToken, setAdminToken] = useState("");
  const [actor, setActor] = useState("operator");

  useEffect(() => {
    setAdminToken(getAdminToken());
    setActor(typeof window !== "undefined" ? (window.localStorage.getItem("arbx-actor") || "operator") : "operator");
  }, []);

  if (initialError) {
    return (
      <Alert variant="destructive">
        <AlertCircleIcon />
        <AlertTitle>resolution endpoint error</AlertTitle>
        <AlertDescription className="font-mono text-xs">{initialError}</AlertDescription>
      </Alert>
    );
  }

  if (!config) {
    return (
      <Alert>
        <AlertCircleIcon />
        <AlertTitle>convergence_config not seeded</AlertTitle>
        <AlertDescription>
          Chain 1 has no trading config row. Seed it via the existing{" "}
          <a href="/config/trading" className="underline">/config/trading</a> form first, then return to this page.
        </AlertDescription>
      </Alert>
    );
  }

  return (
    <Tabs defaultValue="capital-risk" className="w-full">
      <TabsList>
        <TabsTrigger value="capital-risk">Capital &amp; Entropy</TabsTrigger>
        <TabsTrigger value="catalog">Engine Catalog</TabsTrigger>
        <TabsTrigger value="dexes">Exchanges</TabsTrigger>
        <TabsTrigger value="pools">Pools</TabsTrigger>
        <TabsTrigger value="relays">Resolution Relays</TabsTrigger>
        <TabsTrigger value="tokens">Tokens</TabsTrigger>
        <TabsTrigger value="simulation">Simulation</TabsTrigger>
        <TabsTrigger value="audit">Audit Trail</TabsTrigger>
      </TabsList>

      <TabsContent value="capital-risk" className="mt-4">
        <CapitalRiskTab config={config} onSaved={setConfig} adminToken={adminToken} actor={actor} />
      </TabsContent>

      <TabsContent value="catalog" className="mt-4">
        <StrategyCatalogTab
          config={config}
          catalog={initialCatalog}
          onSaved={setConfig}
          adminToken={adminToken}
          actor={actor}
        />
      </TabsContent>

      <TabsContent value="dexes" className="mt-4">
        <DexesTab config={config} onSaved={setConfig} adminToken={adminToken} actor={actor} />
      </TabsContent>

      <TabsContent value="pools" className="mt-4">
        <PoolsTab chainId={config.chain_id} />
      </TabsContent>

      <TabsContent value="relays" className="mt-4">
        <MevRelaysTab />
      </TabsContent>

      <TabsContent value="tokens" className="mt-4">
        <TokenAllowlistTab config={config} onSaved={setConfig} adminToken={adminToken} actor={actor} />
      </TabsContent>

      <TabsContent value="simulation" className="mt-4">
        <SimulationTab config={config} onSaved={setConfig} adminToken={adminToken} actor={actor} />
      </TabsContent>

      <TabsContent value="audit" className="mt-4">
        <AuditTab />
      </TabsContent>
    </Tabs>
  );
}
