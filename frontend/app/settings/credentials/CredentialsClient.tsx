"use client";

/**
 * /settings/credentials — interactive credential editor.
 *
 * UX:
 *   - Sidebar of categories (Hot-path RPC · Price oracles · MEV submission ·
 *     CEX · External · Internal tokens). Active category renders its fields.
 *   - Each field is a CredentialCard with: input(s), test button, status
 *     badge (gray=untested · yellow=…in flight · green=valid · red=invalid),
 *     last-validated timestamp, last error (when invalid).
 *   - Mutations PUT through /admin/credentials. The PUT runs the validator
 *     server-side and persists the status atomically — what we render is
 *     R8 fail-honest: a row is green ONLY after the server executed the
 *     live test and it passed.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { hasAdminSession } from "@/lib/admin-token";
import { rehydrateSystemStore, useSystemStore } from "@/store/useSystemStore";

export interface CredentialItem {
  id: string;
  provider: string;
  scope: string;
  display_name: string;
  has_value: boolean;
  value_suffix: string;
  status: "untested" | "valid" | "invalid" | "expired";
  last_validated_at: string | null;
  last_validation_error: string | null;
  metadata: Record<string, unknown>;
  updated_at: string;
  updated_by: string | null;
}

export interface CredentialsSnapshot {
  items: CredentialItem[];
  ts: string;
  error: string | null;
}

interface CredentialSpec {
  provider: string;
  scope: string;
  display_name: string;
  description: string;
  secret_label: string;
  secret_placeholder: string;
  secret_kind: "password" | "text";
  // Optional metadata fields (e.g. api_key for Binance, url for Titan).
  metadata_fields?: Array<{
    key: string;
    label: string;
    placeholder: string;
    kind: "text" | "password";
  }>;
}

interface CategorySpec {
  id: string;
  label: string;
  description: string;
  creds: CredentialSpec[];
}

// ──────────────────────────────────────────────────────────────────────────
// Catalog — every credential the platform needs, grouped by category.
// ──────────────────────────────────────────────────────────────────────────
const CATEGORIES: CategorySpec[] = [
  {
    id: "rpc",
    label: "Hot-path RPC",
    description: "Ethereum (and additional chains) JSON-RPC + WebSocket providers. Searcher rotates between healthy endpoints. ≥3 providers required for G-RPC-1 readiness.",
    creds: [
      {
        provider: "rpc_http",
        scope: "chain:1",
        display_name: "RPC HTTP — Ethereum mainnet",
        description: "CSV of name=https-url providers. Searcher rotates on failure.",
        secret_label: "Provider CSV",
        secret_placeholder: "publicnode=https://ethereum-rpc.publicnode.com,cloudflare=https://cloudflare-eth.com,drpc=https://eth.drpc.org",
        secret_kind: "text",
      },
      {
        provider: "rpc_ws",
        scope: "chain:1",
        display_name: "RPC WebSocket — Ethereum mainnet",
        description: "CSV of name=wss-url providers. Used by mempool subscription.",
        secret_label: "WS Provider CSV",
        secret_placeholder: "publicnode=wss://ethereum-rpc.publicnode.com,drpc=wss://eth.drpc.org",
        secret_kind: "text",
      },
    ],
  },
  {
    id: "prices",
    label: "Price oracles",
    description: "Token price feeds for USD-denominated yield math. Either Coingecko (free or pro) or Alchemy Prices is sufficient; operator-managed prices in trading_config are the ultimate fallback.",
    creds: [
      {
        provider: "coingecko_demo",
        scope: "global",
        display_name: "Coingecko free tier",
        description: "Free public endpoint, ~30 req/min. Test pings /api/v3/ping.",
        secret_label: "(no key)",
        secret_placeholder: "unused — test pings public endpoint",
        secret_kind: "text",
      },
      {
        provider: "coingecko_pro",
        scope: "global",
        display_name: "Coingecko Pro",
        description: "Paid tier, higher rate limit. Header x-cg-pro-api-key.",
        secret_label: "Pro API Key",
        secret_placeholder: "CG-...",
        secret_kind: "password",
      },
      {
        provider: "alchemy_prices",
        scope: "global",
        display_name: "Alchemy Token Prices API",
        description: "Per-token USD lookup. Test queries WETH price.",
        secret_label: "API key (or full URL with /v2/<key>/...)",
        secret_placeholder: "ABcDeFgHiJkLmNoPqRsTuVwXyZ012345",
        secret_kind: "password",
      },
    ],
  },
  {
    id: "mev",
    label: "MEV submission",
    description: "Private relay endpoints. Flashbots signer is required for bundle submission. BloxRoute / Titan are optional alternative relays.",
    creds: [
      {
        provider: "flashbots_signer",
        scope: "global",
        display_name: "Flashbots searcher signing key",
        description: "0x-prefixed 32-byte private key. Used to sign Flashbots bundle headers. The on-chain executor uses a different key.",
        secret_label: "Private key (0x...)",
        secret_placeholder: "<empty-secret-placeholder>",
        secret_kind: "password",
      },
      {
        provider: "bloxroute",
        scope: "global",
        display_name: "BloxRoute Authorization",
        description: "Authorization header value. Test hits api.bloxroute.com/ping.",
        secret_label: "Authorization header",
        secret_placeholder: "abcd1234-...",
        secret_kind: "password",
      },
      {
        provider: "titan",
        scope: "global",
        display_name: "Titan Builder",
        description: "Builder URL + auth header. Test posts eth_blockNumber.",
        secret_label: "Authorization header",
        secret_placeholder: "Bearer ...",
        secret_kind: "password",
        metadata_fields: [
          { key: "url", label: "Builder URL", placeholder: "https://rpc.titanbuilder.xyz", kind: "text" },
        ],
      },
    ],
  },
  {
    id: "cex",
    label: "CEX APIs (CEX-DEX strategy)",
    description: "Centralized exchange API keys for the CEX-DEX arb strategy. Only read endpoints (account balance) are exercised by the validator.",
    creds: [
      {
        provider: "binance",
        scope: "global",
        display_name: "Binance Spot",
        description: "API key + secret. Validator hits /api/v3/account read-only.",
        secret_label: "API Secret",
        secret_placeholder: "(hidden)",
        secret_kind: "password",
        metadata_fields: [
          { key: "api_key", label: "API Key", placeholder: "abcd1234...", kind: "text" },
        ],
      },
      {
        provider: "okx",
        scope: "global",
        display_name: "OKX",
        description: "API key + secret + passphrase. Validator hits /api/v5/account/balance.",
        secret_label: "API Secret",
        secret_placeholder: "(hidden)",
        secret_kind: "password",
        metadata_fields: [
          { key: "api_key", label: "API Key", placeholder: "abcd1234...", kind: "text" },
          { key: "passphrase", label: "Passphrase", placeholder: "(hidden)", kind: "password" },
        ],
      },
      {
        provider: "bybit",
        scope: "global",
        display_name: "Bybit Unified",
        description: "API key + secret. Validator hits /v5/account/wallet-balance.",
        secret_label: "API Secret",
        secret_placeholder: "(hidden)",
        secret_kind: "password",
        metadata_fields: [
          { key: "api_key", label: "API Key", placeholder: "abcd1234...", kind: "text" },
        ],
      },
    ],
  },
  {
    id: "external",
    label: "External APIs",
    description: "Auxiliary services: GitHub for token-enricher rate-limit bypass (TrustWallet asset feed).",
    creds: [
      {
        provider: "github_token",
        scope: "global",
        display_name: "GitHub Personal Access Token",
        description: "Increases TrustWallet API limits in token-enricher. Validator hits /user.",
        secret_label: "PAT",
        secret_placeholder: "ghp_...",
        secret_kind: "password",
      },
    ],
  },
  {
    id: "internal",
    label: "Internal auth tokens",
    description: "Rotation surface for ARBX_ADMIN_TOKEN + ARBX_EDGE_TOKEN. Validator runs a shape + entropy check (no remote call).",
    creds: [
      {
        provider: "admin_token",
        scope: "global",
        display_name: "ARBX_ADMIN_TOKEN",
        description: "Operator admin token. Must be ≥32 chars, ≥12 distinct, not a placeholder.",
        secret_label: "Admin token",
        secret_placeholder: "(hidden)",
        secret_kind: "password",
      },
      {
        provider: "edge_token",
        scope: "global",
        display_name: "ARBX_EDGE_TOKEN",
        description: "Edge↔api-server shared secret. Same shape requirements as admin token.",
        secret_label: "Edge token",
        secret_placeholder: "(hidden)",
        secret_kind: "password",
      },
    ],
  },
];

const RPC_CATEGORY_TEMPLATE = CATEGORIES.find((cat) => cat.id === "rpc")!;

function buildDynamicRpcCategory(activeChains: Array<{ chainId: number; name: string; rpcHttpHost: string; rpcWsHost: string }>): CategorySpec {
  if (activeChains.length === 0) {
    return {
      ...RPC_CATEGORY_TEMPLATE,
      description: "Primero publica una topología válida en Topology Vault. Esta categoría se hidrata dinámicamente desde el store persistente y no duplica RPC/WSS.",
      creds: [],
    };
  }

  return {
    ...RPC_CATEGORY_TEMPLATE,
    description: "RPC/WSS derivado de Topology Vault. Cada input se genera desde activeChains.map(...) para conservar Topology Vault como SSOT.",
    creds: activeChains.flatMap((chain) => [
      {
        provider: "rpc_http",
        scope: `chain:${chain.chainId}`,
        display_name: `RPC HTTP — ${chain.name}`,
        description: `Scope validado por Topology Vault. Host activo: ${chain.rpcHttpHost || "pendiente"}.`,
        secret_label: "Provider CSV",
        secret_placeholder: chain.rpcHttpHost ? `primary=https://${chain.rpcHttpHost}/...` : "provider=https://...",
        secret_kind: "text" as const,
      },
      {
        provider: "rpc_ws",
        scope: `chain:${chain.chainId}`,
        display_name: `RPC WebSocket — ${chain.name}`,
        description: `Scope validado por Topology Vault. Host activo: ${chain.rpcWsHost || "pendiente"}.`,
        secret_label: "WS Provider CSV",
        secret_placeholder: chain.rpcWsHost ? `primary=wss://${chain.rpcWsHost}/...` : "provider=wss://...",
        secret_kind: "text" as const,
      },
    ]),
  };
}

// ──────────────────────────────────────────────────────────────────────────
// Status badge helper
// ──────────────────────────────────────────────────────────────────────────
function statusBadge(status: CredentialItem["status"] | "in_flight"): {
  label: string;
  className: string;
  dot: string;
} {
  switch (status) {
    case "valid":
      return {
        label: "VALID",
        className: "bg-success/10 border-success/40 text-success",
        dot: "bg-success",
      };
    case "invalid":
      return {
        label: "INVALID",
        className: "bg-destructive/10 border-destructive/40 text-destructive",
        dot: "bg-destructive",
      };
    case "expired":
      return {
        label: "EXPIRED",
        className: "bg-warning/10 border-warning/40 text-warning",
        dot: "bg-warning",
      };
    case "in_flight":
      return {
        label: "TESTING…",
        className: "bg-info/10 border-info/40 text-info",
        dot: "bg-info animate-pulse",
      };
    default:
      return {
        label: "UNTESTED",
        className: "bg-muted/40 border-border text-muted-foreground",
        dot: "bg-muted-foreground",
      };
  }
}

interface CredentialCardProps {
  spec: CredentialSpec;
  current: CredentialItem | null;
  edgeUrl: string;
  onSaved: () => void;
}

function CredentialCard({ spec, current, edgeUrl, onSaved }: CredentialCardProps) {
  const setCredentialStatus = useSystemStore((state) => state.setCredentialStatus);
  const [secret, setSecret] = useState("");
  const [metadata, setMetadata] = useState<Record<string, string>>({});
  const [inFlight, setInFlight] = useState<"none" | "testing" | "saving">("none");
  const [transientStatus, setTransientStatus] = useState<
    "untested" | "valid" | "invalid" | "in_flight" | null
  >(null);
  const [transientMessage, setTransientMessage] = useState<string | null>(null);

  // Hydrate metadata from current row (for non-secret fields like api_key).
  useEffect(() => {
    if (current?.metadata) {
      const md: Record<string, string> = {};
      for (const f of spec.metadata_fields ?? []) {
        const v = current.metadata[f.key];
        if (typeof v === "string") md[f.key] = v;
      }
      setMetadata(md);
    }
  }, [current, spec.metadata_fields]);

  const effectiveStatus = transientStatus ?? current?.status ?? "untested";
  const badge = statusBadge(inFlight === "testing" ? "in_flight" : effectiveStatus);

  const handleTest = useCallback(async () => {
    if (!secret) {
      toast.error("Enter a value before testing");
      return;
    }
    setInFlight("testing");
    setTransientStatus("in_flight");
    setTransientMessage(null);
    try {
      const res = await fetch(`${edgeUrl}/admin/credentials/test`, {
        method: "POST",
        credentials: "include",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          provider: spec.provider,
          scope: spec.scope,
          secret_value: secret,
          metadata,
        }),
      });
      const j = (await res.json()) as { status?: string; message?: string };
      if (!res.ok) {
        setTransientStatus("invalid");
        setTransientMessage(j.message ?? `HTTP ${res.status}`);
        toast.error("Test failed", { description: j.message ?? `HTTP ${res.status}` });
        return;
      }
      const ok = j.status === "valid";
      const nextStatus = ok ? "valid" : "invalid";
      setTransientStatus(nextStatus);
      setTransientMessage(j.message ?? null);
      setCredentialStatus(spec.provider, spec.scope, nextStatus, ok ? new Date().toISOString() : null);
      if (ok) toast.success(`${spec.display_name}`, { description: j.message });
      else toast.error(`${spec.display_name}`, { description: j.message ?? "invalid" });
    } catch (e) {
      setTransientStatus("invalid");
      setTransientMessage((e as Error).message);
      toast.error("Test error", { description: (e as Error).message });
    } finally {
      setInFlight("none");
    }
  }, [edgeUrl, metadata, secret, setCredentialStatus, spec.display_name, spec.provider, spec.scope]);

  const handleSave = useCallback(async () => {
    if (!secret) {
      toast.error("Enter a value before saving");
      return;
    }
    setInFlight("saving");
    try {
      const res = await fetch(`${edgeUrl}/admin/credentials`, {
        method: "PUT",
        credentials: "include",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          provider: spec.provider,
          scope: spec.scope,
          display_name: spec.display_name,
          secret_value: secret,
          metadata,
        }),
      });
      const j = (await res.json()) as { status?: string; last_validation_error?: string };
      if (!res.ok) {
        toast.error("Save failed", { description: j.last_validation_error ?? `HTTP ${res.status}` });
        return;
      }
      setSecret(""); // clear the textbox after save so the suffix shows the persisted state.
      setTransientStatus(null);
      setTransientMessage(null);
      const ok = j.status === "valid";
      setCredentialStatus(spec.provider, spec.scope, ok ? "valid" : "invalid", ok ? new Date().toISOString() : null);
      if (ok) toast.success(`${spec.display_name} saved + validated`);
      else toast.message(`${spec.display_name} saved`, { description: j.last_validation_error ?? "validation failed" });
      onSaved();
    } catch (e) {
      toast.error("Save error", { description: (e as Error).message });
    } finally {
      setInFlight("none");
    }
  }, [edgeUrl, metadata, secret, setCredentialStatus, spec.display_name, spec.provider, spec.scope, onSaved]);

  const handleDelete = useCallback(async () => {
    if (!current) return;
    if (!confirm(`Delete ${spec.display_name}?`)) return;
    try {
      const res = await fetch(
        `${edgeUrl}/admin/credentials/${encodeURIComponent(spec.provider)}/${encodeURIComponent(spec.scope)}`,
        { method: "DELETE", credentials: "include" },
      );
      if (!res.ok) {
        toast.error("Delete failed", { description: `HTTP ${res.status}` });
        return;
      }
      toast.success(`${spec.display_name} deleted`);
      onSaved();
    } catch (e) {
      toast.error("Delete error", { description: (e as Error).message });
    }
  }, [current, edgeUrl, onSaved, spec.display_name, spec.provider, spec.scope]);

  return (
    <div className="bg-card border border-border rounded-xl p-5 space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1">
          <h3 className="text-base font-semibold flex items-center gap-3">
            {spec.display_name}
            <span className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-bold tracking-wide border ${badge.className}`}>
              <span className={`size-1.5 rounded-full ${badge.dot}`} aria-hidden="true" />
              {badge.label}
            </span>
          </h3>
          <p className="text-xs text-muted-foreground mt-1">{spec.description}</p>
        </div>
      </div>

      {current && current.has_value && (
        <div className="text-xs text-muted-foreground flex items-center gap-3 font-mono">
          <span>stored: <code className="px-1 py-0.5 bg-muted rounded">{current.value_suffix}</code></span>
          {current.last_validated_at && (
            <span>tested: {new Date(current.last_validated_at).toLocaleString()}</span>
          )}
        </div>
      )}

      {(transientMessage || current?.last_validation_error) && (
        <div className={`text-xs font-mono p-2 rounded border ${
          effectiveStatus === "valid"
            ? "bg-success/5 border-success/30 text-success"
            : "bg-destructive/5 border-destructive/30 text-destructive"
        }`}>
          {transientMessage ?? current?.last_validation_error}
        </div>
      )}

      <div className="space-y-2">
        {(spec.metadata_fields ?? []).map((f) => (
          <div key={f.key}>
            <label className="text-xs text-muted-foreground block mb-1">{f.label}</label>
            <input
              type={f.kind === "password" ? "password" : "text"}
              placeholder={f.placeholder}
              value={metadata[f.key] ?? ""}
              onChange={(e) => setMetadata({ ...metadata, [f.key]: e.target.value })}
              className="w-full px-3 py-1.5 bg-muted/30 border border-border rounded text-sm font-mono"
            />
          </div>
        ))}
        <div>
          <label className="text-xs text-muted-foreground block mb-1">{spec.secret_label}</label>
          <input
            type={spec.secret_kind === "password" ? "password" : "text"}
            placeholder={spec.secret_placeholder}
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            className="w-full px-3 py-1.5 bg-muted/30 border border-border rounded text-sm font-mono"
            disabled={spec.provider === "coingecko_demo"}
          />
        </div>
      </div>

      <div className="flex items-center gap-2 pt-1">
        <button
          type="button"
          onClick={handleTest}
          disabled={inFlight !== "none" || (spec.provider !== "coingecko_demo" && !secret)}
          className="px-3 py-1.5 rounded text-xs font-semibold bg-info/10 border border-info/40 text-info hover:bg-info/20 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {inFlight === "testing" ? "Testing…" : "Test"}
        </button>
        <button
          type="button"
          onClick={handleSave}
          disabled={inFlight !== "none" || !secret}
          className="px-3 py-1.5 rounded text-xs font-semibold bg-primary/10 border border-primary/40 text-primary hover:bg-primary/20 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {inFlight === "saving" ? "Saving…" : "Save + validate"}
        </button>
        {current?.has_value && (
          <button
            type="button"
            onClick={handleDelete}
            className="ml-auto px-3 py-1.5 rounded text-xs font-semibold bg-destructive/10 border border-destructive/40 text-destructive hover:bg-destructive/20"
          >
            Delete
          </button>
        )}
      </div>
    </div>
  );
}

interface ClientProps {
  initialSnapshot: CredentialsSnapshot;
  edgeUrl: string;
}

export function CredentialsClient({ initialSnapshot, edgeUrl }: ClientProps) {
  const router = useRouter();
  const [snapshot, setSnapshot] = useState<CredentialsSnapshot>(initialSnapshot);
  const [activeCategory, setActiveCategory] = useState<string>(CATEGORIES[0]!.id);
  const [isMounted, setIsMounted] = useState(false);
  const [hasSession, setHasSession] = useState(false);
  const activeChains = useSystemStore((state) => state.topology.activeChains);

  useEffect(() => {
    rehydrateSystemStore();
    setIsMounted(true);
    setHasSession(hasAdminSession());
  }, []);

  const refresh = useCallback(async () => {
    try {
      const res = await fetch(`${edgeUrl}/api/credentials`, {
        credentials: "include",
        cache: "no-store",
      });
      if (!res.ok) {
        setSnapshot({ ...snapshot, error: `HTTP ${res.status}` });
        return;
      }
      const data = (await res.json()) as { items?: CredentialItem[]; ts?: string };
      setSnapshot({
        items: data.items ?? [],
        ts: data.ts ?? new Date().toISOString(),
        error: null,
      });
    } catch (e) {
      setSnapshot({ ...snapshot, error: (e as Error).message });
    }
  }, [edgeUrl, snapshot]);

  const dynamicRpcCategory = useMemo(() => buildDynamicRpcCategory(activeChains), [activeChains]);

  const categories = useMemo(() => [
    dynamicRpcCategory,
    ...CATEGORIES.filter((cat) => cat.id !== "rpc"),
  ], [dynamicRpcCategory]);

  const byKey = useMemo(() => {
    const map = new Map<string, CredentialItem>();
    for (const it of snapshot.items) {
      map.set(`${it.provider}:${it.scope}`, it);
    }
    return map;
  }, [snapshot.items]);

  const summary = useMemo(() => {
    let total = 0, valid = 0, invalid = 0, untested = 0;
    for (const cat of categories) {
      for (const c of cat.creds) {
        total += 1;
        const row = byKey.get(`${c.provider}:${c.scope}`);
        if (!row || !row.has_value) untested += 1;
        else if (row.status === "valid") valid += 1;
        else if (row.status === "invalid") invalid += 1;
        else untested += 1;
      }
    }
    return { total, valid, invalid, untested };
  }, [byKey, categories]);

  const activeCat = categories.find((c) => c.id === activeCategory) ?? categories[0]!;

  if (isMounted && !hasSession) {
    return (
      <div className="p-8 min-h-screen">
        <h1 className="text-3xl font-bold mb-4">Credentials</h1>
        <div className="bg-warning/10 border border-warning/40 text-warning rounded-lg p-4">
          <p className="font-semibold">Admin session required</p>
          <p className="text-sm mt-1">
            Go to <button onClick={() => router.push("/admin/signin?next=/settings/credentials")} className="underline">/admin/signin</button> to sign in with your admin token first.
          </p>
          <p className="text-sm mt-3">
            For Alchemy, PublicNode or any RPC/WSS provider used by the hot path, use <button onClick={() => router.push("/admin/signin?next=/admin/topology")} className="font-semibold underline">/admin/topology</button> after signing in. That screen sends full URLs only to the API Server and shows back masked provider snapshots.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8 min-h-screen">
      <div className="border-b border-border pb-4 mb-6">
        <h1 className="text-3xl font-bold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-primary to-success">
          Credentials
        </h1>
        <p className="text-muted-foreground mt-1 text-sm">
          Operator-managed external credentials. Each row turns green only after the server runs the live test and it passes (R8 fail-honest).
        </p>
        <div className="mt-3 rounded-lg border border-primary/30 bg-primary/10 p-3 text-sm text-foreground">
          <span className="font-semibold">Hot-path RPC/WSS:</span> configure Alchemy, PublicNode or other node providers in <button onClick={() => router.push("/admin/topology")} className="font-semibold underline">Topology Vault</button>. This generic Credentials page remains for non-topology secrets and legacy checks.
        </div>
        <div className="flex items-center gap-4 mt-3 text-sm">
          <span className="text-success font-semibold">{summary.valid} valid</span>
          <span className="text-destructive font-semibold">{summary.invalid} invalid</span>
          <span className="text-muted-foreground">{summary.untested} untested</span>
          <span className="text-muted-foreground">/ {summary.total} total</span>
          <button onClick={refresh} className="ml-auto px-3 py-1 rounded text-xs bg-muted hover:bg-accent border border-border">
            Refresh
          </button>
        </div>
        {snapshot.error && (
          <div className="text-xs text-destructive mt-2">Last fetch error: {snapshot.error}</div>
        )}
      </div>

      <div className="grid grid-cols-[220px_1fr] gap-6">
        <aside className="space-y-1">
          {categories.map((cat) => {
            const catRows = cat.creds.map((c) => byKey.get(`${c.provider}:${c.scope}`));
            const validInCat = catRows.filter((r) => r?.status === "valid").length;
            const invalidInCat = catRows.filter((r) => r?.status === "invalid").length;
            return (
              <button
                key={cat.id}
                type="button"
                onClick={() => setActiveCategory(cat.id)}
                className={`w-full text-left px-3 py-2 rounded-lg border transition-colors ${
                  activeCategory === cat.id
                    ? "bg-primary/10 border-primary/40"
                    : "bg-card border-border hover:bg-muted"
                }`}
              >
                <div className="font-semibold text-sm">{cat.label}</div>
                <div className="text-xs text-muted-foreground mt-0.5 flex items-center gap-2">
                  <span>{cat.creds.length} cred</span>
                  {validInCat > 0 && <span className="text-success">{validInCat}✓</span>}
                  {invalidInCat > 0 && <span className="text-destructive">{invalidInCat}✗</span>}
                </div>
              </button>
            );
          })}
        </aside>

        <section className="space-y-4">
          <div className="bg-muted/20 border border-border rounded-lg p-3 text-sm text-muted-foreground">
            {activeCat.description}
          </div>
          {activeCat.id === "rpc" && activeCat.creds.length === 0 && (
            <div className="rounded-xl border border-warning/40 bg-warning/10 p-5 text-sm text-foreground">
              Publica primero una topología activa en <button onClick={() => router.push("/admin/topology")} className="font-semibold underline">Topology Vault</button>. La categoría RPC se generará automáticamente desde el store persistente sin pedir cadenas duplicadas.
            </div>
          )}
          {activeCat.creds.map((spec) => (
            <CredentialCard
              key={`${spec.provider}:${spec.scope}`}
              spec={spec}
              current={byKey.get(`${spec.provider}:${spec.scope}`) ?? null}
              edgeUrl={edgeUrl}
              onSaved={refresh}
            />
          ))}
        </section>
      </div>
    </div>
  );
}
