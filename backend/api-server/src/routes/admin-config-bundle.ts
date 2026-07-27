/**
 * admin-config-bundle — Ruta 2 (HTTP upload) for the Encrypted Config Bundle.
 *
 * The operator's Excel macro (ArbxBundleShipper.ShipBundle) generates the .enc;
 * this router is the browser-upload path. SSH upload (Ruta 1) lands the SAME
 * .enc on the SAME path via scp — both routes then trigger the SAME importer
 * binary (bundle_importer), so there is one decrypt/apply path, two triggers.
 *
 * Endpoints (all gated by V-AT-1 admin token, all audited):
 *
 *   POST /api/admin/config/upload-bundle
 *     Body: { enc_base64: string, sha256?: string }
 *     Decodes, writes /opt/.../config/arbx_config_bundle.json.enc (atomic temp
 *     + rename), returns { ok, path, size, sha256 }. Never runs the importer.
 *
 *   POST /api/admin/config/run-importer
 *     Shells bundle_importer --enc <path> --private-key <pem> --schema <json>
 *     --apply [--dry-run]. Streams the importer's JSON report back. Reuses the
 *     "Recargar" trigger pattern from RpcSyncPanel (operator-gated, on-demand).
 *
 *   GET  /api/admin/config/bundle-status
 *     Does the .enc exist? size, mtime, sha256 (no plaintext, no secrets).
 *
 * Doctrinal invariants:
 *  - paper_mode never touched (the importer owns that assertion; the endpoint
 *    only ferries the opaque .enc bytes — it cannot read them, no private key).
 *  - Capital $0 — no contract calls, no signer, no executor.
 *  - Fail-honest (R8) — every failure returns a typed 4xx/5xx + diagnostic.
 *  - The private key NEVER enters this process — only the importer binary
 *    reads it, in a separate process. Defense-in-depth: even if the api-server
 *    is compromised, the private key is one hop away in a different binary.
 */
import type { Application, Request, Response } from "express";
import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { readFile, writeFile, rename, stat } from "node:fs/promises";
import { existsSync } from "node:fs";
import { promisify } from "node:util";
import { z } from "zod";

const execFileP = promisify(execFile);

// ---------------------------------------------------------------------------
// Paths — operator-overridable via env, fail-closed defaults for prod.
// ---------------------------------------------------------------------------

const BUNDLE_ENC_PATH =
  process.env.ARBX_BUNDLE_ENC_PATH ?? "/opt/arbitragex-v2/config/arbx_config_bundle.json.enc";
const PRIVATE_KEY_PATH =
  process.env.ARBX_BUNDLE_PRIVATE_KEY ?? "/opt/arbitragex-v2/config/arbx_bundle_private.pem";
const SCHEMA_PATH =
  process.env.ARBX_BUNDLE_SCHEMA_PATH ?? "/opt/arbitragex-v2/scripts/arbx-env-deploy/bundle_schema.json";
const IMPORTER_BIN =
  process.env.ARBX_BUNDLE_IMPORTER_BIN ?? "bundle_importer";

/** Generous cap: a real bundle .enc is ~1-5KB. 1MB absorbs any sane future growth. */
const MAX_ENC_BYTES = 1_000_000;

// ---------------------------------------------------------------------------
// Wire schemas (Zod)
// ---------------------------------------------------------------------------

const UploadBundleSchema = z.object({
  enc_base64: z.string().min(64, "enc_base64 too short to be a real bundle"),
  sha256: z.string().regex(/^[a-f0-9]{64}$/i).optional(),
});

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

export function mountAdminConfigBundle(
  app: Application,
  deps: {
    requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
    adminToken: string;
    writeAudit: (
      action: string,
      actor: string,
      kind: string,
      target: string,
      before: unknown,
      after: unknown,
      ip: string | null,
      traceId: string | null,
      ua: string | null,
    ) => Promise<void>;
    logger: { warn: (obj: object, msg?: string) => void; info: (obj: object, msg?: string) => void };
  },
): void {
  const { requireAdminToken, adminToken, writeAudit, logger } = deps;
  const gate = requireAdminToken(adminToken);

  // POST upload-bundle — write the opaque .enc bytes; NO decrypt here.
  app.post("/api/admin/config/upload-bundle", gate, async (req: Request, res: Response) => {
    const parsed = UploadBundleSchema.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({ error: "invalid_body", issues: parsed.error.issues });
      return;
    }
    let enc: Buffer;
    try {
      enc = Buffer.from(parsed.data.enc_base64, "base64");
    } catch {
      res.status(400).json({ error: "bad_base64" });
      return;
    }
    if (enc.length < 60 || enc.length > MAX_ENC_BYTES) {
      res.status(400).json({
        error: "bad_size",
        size: enc.length,
        allowed: `60..${MAX_ENC_BYTES}`,
      });
      return;
    }
    if (enc.subarray(0, 5).toString("latin1") !== "ARBX1") {
      res.status(400).json({ error: "bad_magic", got: enc.subarray(0, 5).toString("latin1") });
      return;
    }
    const sha256 = createHash("sha256").update(enc).digest("hex");
    if (parsed.data.sha256 && parsed.data.sha256.toLowerCase() !== sha256) {
      res.status(400).json({ error: "sha256_mismatch", declared: parsed.data.sha256, computed: sha256 });
      return;
    }
    // Atomic write: temp in the same dir, then rename.
    const tmpPath = `${BUNDLE_ENC_PATH}.tmp.${process.pid}`;
    try {
      await writeFile(tmpPath, enc, { mode: 0o640 });
      await rename(tmpPath, BUNDLE_ENC_PATH);
      const actor = req.header("x-arbx-actor") ?? "admin";
      await writeAudit(
        "admin.bundle.upload",
        actor,
        "bundle",
        BUNDLE_ENC_PATH,
        null,
        { size: enc.length, sha256 },
        req.ip ?? null,
        (req as unknown as { traceId?: string }).traceId ?? null,
        req.header("user-agent") ?? null,
      );
      logger.info({ event: "admin_bundle.uploaded", size: enc.length, sha256 });
      res.status(200).json({ ok: true, path: BUNDLE_ENC_PATH, size: enc.length, sha256 });
    } catch (e) {
      logger.warn({ event: "admin_bundle.upload_failed", err: (e as Error).message });
      res.status(503).json({ error: "write_failed", detail: (e as Error).message });
    }
  });

  // POST run-importer — shell the Rust binary (one importer, both routes).
  app.post("/api/admin/config/run-importer", gate, async (req: Request, res: Response) => {
    const dryRun = req.query["dry-run"] === "1" || req.body?.dry_run === true;
    const args = [
      "--enc", BUNDLE_ENC_PATH,
      "--private-key", PRIVATE_KEY_PATH,
      "--schema", SCHEMA_PATH,
      dryRun ? "--dry-run" : "--apply",
    ];
    try {
      // Max 30s — the importer is decrypt + idempotent upsert, fast.
      const { stdout, stderr } = await execFileP(IMPORTER_BIN, args, { timeout: 30_000, maxBuffer: 1 << 20 });
      const actor = req.header("x-arbx-actor") ?? "admin";
      await writeAudit(
        `admin.bundle.import${dryRun ? "_dryrun" : ""}`,
        actor,
        "bundle",
        BUNDLE_ENC_PATH,
        null,
        { stdout_head: stdout.slice(0, 200), exit: 0 },
        req.ip ?? null,
        (req as unknown as { traceId?: string }).traceId ?? null,
        req.header("user-agent") ?? null,
      );
      logger.info({ event: "admin_bundle.imported", dry_run: dryRun, stdout_head: stdout.slice(0, 200) });
      // The importer prints a JSON report on stdout; pass it through.
      let report: unknown = null;
      try {
        report = JSON.parse(stdout);
      } catch {
        report = { raw_stdout: stdout.slice(0, 1000), stderr: stderr.slice(0, 500) };
      }
      res.status(200).json({ ok: true, dry_run: dryRun, report });
    } catch (e) {
      const err = e as Error & { code?: number | string; stdout?: string; stderr?: string };
      const exitCode = typeof err.code === "number" ? err.code : null;
      logger.warn({
        event: "admin_bundle.import_failed",
        err: err.message,
        exit_code: exitCode,
        stderr_head: (err.stderr ?? "").slice(0, 200),
      });
      // 422 = the importer ran but rejected the bundle (tamper/schema/NEVER_SHIP).
      // 503 = the importer binary itself couldn't run (not deployed / ENOENT).
      const importerMissing = /ENOENT|not found|spawn/i.test(err.message);
      const status = importerMissing ? 503 : 422;
      res.status(status).json({
        error: importerMissing ? "importer_not_deployed" : "importer_rejected",
        detail: err.message,
        exit_code: exitCode,
        stderr: (err.stderr ?? "").slice(0, 500),
        stdout: (err.stdout ?? "").slice(0, 500),
        hint: importerMissing
          ? `deploy the bundle_importer binary (shared-rs) and set ARBX_BUNDLE_IMPORTER_BIN if not on PATH`
          : "the .enc is tampered, the schema drifted, or a NEVER_SHIP key leaked",
      });
    }
  });

  // GET bundle-status — does the .enc exist? opaque metadata only (no plaintext).
  app.get("/api/admin/config/bundle-status", gate, async (_req: Request, res: Response) => {
    try {
      if (!existsSync(BUNDLE_ENC_PATH)) {
        res.status(200).json({ present: false });
        return;
      }
      const buf = await readFile(BUNDLE_ENC_PATH);
      const st = await stat(BUNDLE_ENC_PATH);
      res.status(200).json({
        present: true,
        path: BUNDLE_ENC_PATH,
        size: buf.length,
        mtime: st.mtime.toISOString(),
        sha256: createHash("sha256").update(buf).digest("hex"),
        magic_ok: buf.subarray(0, 5).toString("latin1") === "ARBX1",
      });
    } catch (e) {
      res.status(503).json({ error: "status_failed", detail: (e as Error).message });
    }
  });
}
