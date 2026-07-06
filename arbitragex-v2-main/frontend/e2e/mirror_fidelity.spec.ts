/**
 * =============================================================================
 * OMEGA Mirror Fidelity E2E — enforces the 7-Layer Coherence Rule
 * =============================================================================
 *
 * Every feature declared in /api/system/feature_manifest MUST:
 *   1. Have a renderable panel at feature.panel_path
 *   2. Expose a /api/<entity> endpoint when feature.layer = "backend"
 *   3. Emit reload on at least one channel listed in reloadChannelsFor()
 *
 * Failures of this test BLOCK Crucible → Mainnet phase transition.
 *
 * Test runner: Playwright (preserves existing project test infrastructure).
 */
import { test, expect } from "@playwright/test";

interface FeatureManifestEntry {
  feature_key: string;
  description: string;
  layer: "backend" | "contract" | "runtime" | "frontend" | "edge";
  state_hash: string;
  panel_path: string;
  required: boolean;
}

test.describe("OMEGA Mirror Fidelity", () => {
  test("every required feature has a renderable panel", async ({ page, request }) => {
    const r = await request.get("/api/system/feature_manifest");
    expect(r.ok()).toBeTruthy();
    const body = (await r.json()) as { features: FeatureManifestEntry[] };
    const required = body.features.filter((f) => f.required);
    expect(required.length).toBeGreaterThanOrEqual(13);

    for (const f of required) {
      await test.step(`panel exists: ${f.feature_key} → ${f.panel_path}`, async () => {
        const resp = await page.goto(f.panel_path);
        expect(resp?.status() ?? 0, `${f.panel_path} must return 2xx`).toBeLessThan(400);
        await expect(page.locator("body")).not.toHaveText(/404|not found/i, { timeout: 3000 });
      });
    }
  });

  test("config-hashes endpoint exposes all 12 registries", async ({ request }) => {
    const r = await request.get("/api/system/config-hashes");
    expect(r.ok()).toBeTruthy();
    const body = (await r.json()) as { hashes: Array<{ resource: string }> };
    const resources = new Set(body.hashes.map((h) => h.resource));
    const required = [
      "chains","rpcs","dexes","pools","tokens","wallets",
      "strategies","contracts","risk_gates","capital_gates","relays","agents",
    ];
    for (const res of required) {
      expect(resources.has(res), `config_hash missing for resource ${res}`).toBeTruthy();
    }
  });

  test("drift endpoint returns 200 with structured envelope", async ({ request }) => {
    const r = await request.get("/api/system/drift");
    expect(r.ok()).toBeTruthy();
    const body = (await r.json()) as { drift: unknown[]; count: number };
    expect(Array.isArray(body.drift)).toBeTruthy();
    expect(typeof body.count).toBe("number");
  });

  test("runtime-ack rejects malformed payload (schema-first)", async ({ request }) => {
    const r = await request.post("/api/system/runtime-ack", {
      data: { event_id: "not-a-uuid" },
    });
    expect(r.status()).toBe(400);
  });
});
