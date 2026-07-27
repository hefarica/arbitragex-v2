// Encrypted Config Bundle panel — Ruta 2 (browser upload). Mirrors RpcSyncPanel's
// admin-gated pattern. The Excel macro (ArbxBundleShipper.ShipBundle) GENERATES the
// .enc; this panel accepts the upload + triggers the importer. SSH (Ruta 1) lands
// the same .enc on the same path — one importer, two triggers.
//
// Zero-mock: renders exactly what bundle-status returns. Empty = empty. No
// fabricated state. The panel NEVER sees plaintext or the private key — it ferries
// opaque .enc bytes; only the importer binary (separate process) decrypts.
"use client";

import React, { useCallback, useEffect, useRef, useState } from "react";
import { UploadCloud, FileCheck2, KeyRound, FlaskConical } from "lucide-react";
import { getApiBaseUrl } from "@/lib/api-client";
import { hasAdminSession } from "@/lib/admin-token";

type BundleStatus = {
  present: boolean;
  path?: string;
  size?: number;
  mtime?: string;
  sha256?: string;
  magic_ok?: boolean;
};

export function BundleSyncPanel() {
  const [status, setStatus] = useState<BundleStatus | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState<null | "upload" | "import" | "dryrun">(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [isAdmin, setIsAdmin] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const loadStatus = useCallback(async () => {
    setErr(null);
    try {
      const res = await fetch(`${getApiBaseUrl()}/api/admin/config/bundle-status`, {
        credentials: "include",
        cache: "no-store",
      });
      if (res.status === 401) {
        setErr("admin session required");
        return;
      }
      if (!res.ok) {
        setErr(`status unreachable (HTTP ${res.status})`);
        return;
      }
      setStatus((await res.json()) as BundleStatus);
    } catch (e) {
      setErr((e as Error).message);
    }
  }, []);

  useEffect(() => {
    setIsAdmin(hasAdminSession());
    void loadStatus();
  }, [loadStatus]);

  const onFile = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      e.target.value = "";
      if (!file) return;
      setBusy("upload");
      setMsg(null);
      try {
        const buf = await file.arrayBuffer();
        const enc_base64 = btoa(String.fromCharCode(...new Uint8Array(buf)));
        const res = await fetch(`${getApiBaseUrl()}/api/admin/config/upload-bundle`, {
          method: "POST",
          credentials: "include",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            enc_base64,
            sha256: await sha256Hex(buf),
          }),
        });
        if (!res.ok) {
          const j = await res.json().catch(() => ({}));
          setMsg(`Upload failed: HTTP ${res.status} — ${j.error ?? res.statusText}${j.error === "sha256_mismatch" ? " (declared vs computed)" : ""}`);
          return;
        }
        const j = (await res.json()) as { size: number; sha256: string };
        setMsg(`Uploaded ${(j.size / 1024).toFixed(1)} KB · sha256 ${j.sha256.slice(0, 12)}…`);
        await loadStatus();
      } catch (e) {
        setMsg(`Upload error: ${(e as Error).message}`);
      } finally {
        setBusy(null);
      }
    },
    [loadStatus],
  );

  const runImporter = useCallback(
    async (dryRun: boolean) => {
      setBusy(dryRun ? "dryrun" : "import");
      setMsg(null);
      try {
        const res = await fetch(`${getApiBaseUrl()}/api/admin/config/run-importer${dryRun ? "?dry-run=1" : ""}`, {
          method: "POST",
          credentials: "include",
          headers: { "content-type": "application/json" },
          body: "{}",
        });
        const j = (await res.json().catch(() => ({}))) as {
          ok?: boolean;
          error?: string;
          detail?: string;
          hint?: string;
          report?: { env_vars?: number; chains?: number; factories?: number; inserted?: number; updated?: number };
        };
        if (!res.ok) {
          setMsg(
            `Import failed: HTTP ${res.status} — ${j.error ?? res.statusText}` +
              (j.error === "importer_not_deployed" ? " (deploy the bundle_importer binary first)" : ""),
          );
          return;
        }
        const r = j.report ?? {};
        setMsg(
          `${dryRun ? "Dry-run OK" : "Imported"}: ${r.chains ?? 0} chain(s) · ${r.factories ?? 0} factories · env_vars ${r.env_vars ?? 0}` +
            (dryRun ? " (no writes)" : ` (+${r.inserted ?? 0} new, ~${r.updated ?? 0} updated)`),
        );
      } catch (e) {
        setMsg(`Import error: ${(e as Error).message}`);
      } finally {
        setBusy(null);
      }
    },
    [],
  );

  return (
    <div data-slot="card" className="bg-card text-card-foreground border border-border rounded-xl shadow-2xl p-5 space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">Encrypted Config Bundle</h2>
          <p className="text-xs text-muted-foreground mt-0.5">
            Ship the full Excel config as one RSA-encrypted bundle. The macro{" "}
            <span className="font-mono">ShipBundle</span> generates the <span className="font-mono">.enc</span>; upload it
            here (Ruta 2) or via SSH (Ruta 1). The importer decrypts on the VPS and applies env_vars, chains, factories
            — <span className="font-mono">paper_mode</span> is never touched.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => fileRef.current?.click()}
            disabled={busy !== null}
            title="Upload the .enc produced by the Excel ShipBundle macro"
            className="inline-flex items-center gap-1.5 rounded-md border border-primary/40 bg-primary/10 px-3 py-1.5 text-sm font-medium text-primary hover:bg-primary/20 disabled:opacity-50"
          >
            <UploadCloud size={14} className={busy === "upload" ? "animate-pulse" : ""} />
            Subir .enc
          </button>
          <button
            type="button"
            onClick={() => runImporter(true)}
            disabled={busy !== null || !status?.present}
            title="Decrypt + validate + report, NO writes"
            className="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted px-3 py-1.5 text-sm font-medium hover:bg-accent disabled:opacity-50"
          >
            <FlaskConical size={14} className={busy === "dryrun" ? "animate-pulse" : ""} />
            Dry-run
          </button>
          <button
            type="button"
            onClick={() => runImporter(false)}
            disabled={busy !== null || !status?.present}
            title="Decrypt + apply (idempotent upsert into chains_runtime / rpc_endpoints / factories + .env)"
            className="inline-flex items-center gap-1.5 rounded-md border border-success/40 bg-success/10 px-3 py-1.5 text-sm font-medium text-success hover:bg-success/20 disabled:opacity-50"
          >
            <FileCheck2 size={14} className={busy === "import" ? "animate-pulse" : ""} />
            Importar en VPS
          </button>
          <input ref={fileRef} type="file" accept=".enc,application/octet-stream" onChange={onFile} className="hidden" />
        </div>
      </div>

      {!isAdmin && (
        <div className="flex items-center gap-2 rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-xs text-warning">
          <KeyRound size={14} />
          Admin session required for upload / import.{" "}
          <a href="/admin/signin?next=/rpcs" className="font-semibold underline">
            Sign in
          </a>
        </div>
      )}

      {err ? (
        <p className="text-sm text-destructive">Status: {err}</p>
      ) : !status ? (
        <p className="text-sm text-muted-foreground">Loading bundle status…</p>
      ) : !status.present ? (
        <p className="text-sm text-muted-foreground">
          No bundle on the VPS yet. Run the <span className="font-mono">ShipBundle</span> macro in Excel, then upload the{" "}
          <span className="font-mono">.enc</span> here.
        </p>
      ) : (
        <div className="rounded-md border border-border bg-muted/30 px-3 py-2 text-sm space-y-1">
          <div className="flex items-center gap-2">
            <FileCheck2 size={14} className="text-success" />
            <span className="font-mono">{((status.size ?? 0) / 1024).toFixed(1)} KB</span>
            <span className="text-muted-foreground">· {(status.mtime ?? "").replace("T", " ").slice(0, 19)}Z</span>
            {status.magic_ok === false && (
              <span className="text-destructive text-xs">· BAD MAGIC (not an ARBX1 bundle)</span>
            )}
          </div>
          <div className="text-xs text-muted-foreground font-mono truncate">sha256: {status.sha256 ?? "—"}</div>
        </div>
      )}

      {msg && <p className="text-sm font-medium text-foreground">{msg}</p>}
    </div>
  );
}

/** WebCrypto SHA-256 → hex. Runs in the browser; the same hash the server computes. */
async function sha256Hex(buf: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", buf);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}
