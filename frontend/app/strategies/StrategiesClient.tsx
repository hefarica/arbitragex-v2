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
import Link from "next/link";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
// FE-0041 (§52-§54): per-tab scope labels. Classification verified by write
// grep (putTradingConfig / admin POST-PUT) — see
// docs/reports/2026-08-24-FE-0041-control-scope-audit.md.
import { ControlScopeBadge } from "@/components/ControlScopeBadge";
import { getAdminToken } from "@/lib/admin-token";
import type { StrategyCatalogEntry, TradingConfigConfigured, TradingConfigResponse } from "@/lib/schemas";

import { CapitalRiskTab } from "./tabs/CapitalRiskTab";
import { EngineCatalogClient } from "./EngineCatalogClient";
import { RuntimeCartridgesTab } from "./tabs/RuntimeCartridgesTab";
import { MathOperatorsTab } from "./tabs/MathOperatorsTab";
import { DexesTab } from "./tabs/DexesTab";
import { PoolsTab } from "./tabs/PoolsTab";
import { MevRelaysTab } from "./tabs/MevRelaysTab";
import { DetectorPolicyPanel } from "./tabs/DetectorPolicyPanel";
import { TokensTabClient } from "./tabs/TokensTabClient";
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
          <Link href="/config/trading" className="underline">/config/trading</Link> form first, then return to this page.
        </AlertDescription>
      </Alert>
    );
  }

  return (
    <Tabs defaultValue="capital-risk" className="w-full">
      {/* 48_SURFACE_CERT Responsive: 11 triggers overflow w-fit at <1280px —
          wrap instead of overflowing (TabsList primitive untouched). */}
      <TabsList className="h-auto flex-wrap justify-start">
        <TabsTrigger value="capital-risk">Capital &amp; Entropy</TabsTrigger>
        <TabsTrigger value="catalog">Engine Catalog</TabsTrigger>
        <TabsTrigger value="runtime">Runtime Cartridges</TabsTrigger>
        <TabsTrigger value="math">Math Operators</TabsTrigger>
        <TabsTrigger value="dexes">Exchanges</TabsTrigger>
        <TabsTrigger value="pools">Pools</TabsTrigger>
        <TabsTrigger value="relays">Resolution Relays</TabsTrigger>
        <TabsTrigger value="detectors">Detector Policy</TabsTrigger>
        <TabsTrigger value="tokens">Tokens</TabsTrigger>
        <TabsTrigger value="simulation">Simulation</TabsTrigger>
        <TabsTrigger value="audit">Audit Trail</TabsTrigger>
      </TabsList>

      <TabsContent value="capital-risk" className="mt-4">
        <ControlScopeBadge kind="RUNTIME_MUTATION" className="mb-2" />
        <CapitalRiskTab config={config} onSaved={setConfig} adminToken={adminToken} actor={actor} />
      </TabsContent>

      {/* FE-MASTER §21-§24 (P6): Engine Catalog gains the Workbook-264 canon
          view next to the runtime-kinds view (EMIT-07 consumer). */}
      <TabsContent value="catalog" className="mt-4">
        {/* Catalog mixes the canon VIEW with the mutating runtime-kinds list
            (StrategyCatalogTab → putTradingConfig). */}
        <ControlScopeBadge kind="RUNTIME_MUTATION" className="mb-2" />
        <EngineCatalogClient
          config={config}
          catalog={initialCatalog}
          onSaved={setConfig}
          adminToken={adminToken}
          actor={actor}
        />
      </TabsContent>

      <TabsContent value="runtime" className="mt-4">
        <ControlScopeBadge kind="RUNTIME_MUTATION" className="mb-2" />
        <RuntimeCartridgesTab
          chainId={config.chain_id}
          config={config}
          onSaved={setConfig}
          adminToken={adminToken}
          actor={actor}
        />
      </TabsContent>

      <TabsContent value="math" className="mt-4">
        <ControlScopeBadge kind="VIEW_ONLY" className="mb-2" />
        <MathOperatorsTab adminToken={adminToken} actor={actor} />
      </TabsContent>

      <TabsContent value="dexes" className="mt-4">
        <ControlScopeBadge kind="RUNTIME_MUTATION" className="mb-2" />
        <DexesTab config={config} onSaved={setConfig} adminToken={adminToken} actor={actor} />
      </TabsContent>

      <TabsContent value="pools" className="mt-4">
        <ControlScopeBadge kind="VIEW_ONLY" className="mb-2" />
        <PoolsTab chainId={config.chain_id} />
      </TabsContent>

      <TabsContent value="relays" className="mt-4">
        <ControlScopeBadge kind="VIEW_ONLY" className="mb-2" />
        <MevRelaysTab />
      </TabsContent>

      {/* FE-MASTER ARBX-DP-005: the four emission tiers (OBSERVATION /
          SIGNAL / CANDIDATE / EXECUTABLE) as distinct feeds over the
          detector policy catalog (EMIT-08). */}
      <TabsContent value="detectors" className="mt-4">
        <ControlScopeBadge kind="VIEW_ONLY" className="mb-2" />
        <DetectorPolicyPanel />
      </TabsContent>

      <TabsContent value="tokens" className="mt-4">
        {/* FE-MASTER §6: sub-vistas Universe | Quote/Base | Pair Intelligence
            (FE-0017) — segmented control, no nested Radix Tabs. The tab is
            RUNTIME_MUTATION because its allowlist subvista saves via
            putTradingConfig (TokenAllowlistTab). */}
        <ControlScopeBadge kind="RUNTIME_MUTATION" className="mb-2" />
        <TokensTabClient config={config} onSaved={setConfig} adminToken={adminToken} actor={actor} />
      </TabsContent>

      <TabsContent value="simulation" className="mt-4">
        <ControlScopeBadge kind="RUNTIME_MUTATION" className="mb-2" />
        <SimulationTab config={config} onSaved={setConfig} adminToken={adminToken} actor={actor} />
      </TabsContent>

      <TabsContent value="audit" className="mt-4">
        <ControlScopeBadge kind="VIEW_ONLY" className="mb-2" />
        <AuditTab />
      </TabsContent>
    </Tabs>
  );
}
