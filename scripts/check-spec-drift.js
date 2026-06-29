#!/usr/bin/env node
/**
 * FASE 4 Inc 2 — anti-regression drift assertions for apis/*.yaml.
 *
 * Pure Node (no dependencies). Fails CI if any contract regression that FASE 4
 * already proved was real reappears (leaked staging IP, wrong admin auth header,
 * the fictional raw-WebSocket channel model), or if a corrected core public path
 * disappears. This is a bounded guardrail, not a full runtime/spec equivalence
 * engine.
 */
const fs = require("node:fs");
const path = require("node:path");

const apis = path.join(process.cwd(), "apis");
const openapi = fs.readFileSync(path.join(apis, "openapi.yaml"), "utf8");
const asyncapi = fs.readFileSync(path.join(apis, "asyncapi.yaml"), "utf8");
const combined = `${openapi}\n${asyncapi}`;

const fails = [];
const mustNot = (hay, needle, msg) => { if (hay.includes(needle)) fails.push(msg); };
const must = (hay, needle, msg) => { if (!hay.includes(needle)) fails.push(msg); };

// 1. No leaked staging IP in the published specs.
mustNot(combined, "195.201.235.70", "leaked staging IP 195.201.235.70 must not appear in apis/");

// 2. Admin auth header must be the real one. Match on `name: x-admin-token`, NOT
//    bare `x-admin-token` — the latter is a substring of the correct
//    `x-arbx-admin-token` and would false-positive.
mustNot(combined, "name: x-admin-token", "wrong admin auth header `x-admin-token` must not be the published contract");
must(combined, "x-arbx-admin-token", "real admin auth header `x-arbx-admin-token` must be present");

// 3. Fictional raw-WebSocket channel addresses must not reappear (reality is
//    Socket.IO rooms on the default namespace, not /ws/* paths).
mustNot(asyncapi, "/ws/opportunities", "fictional raw-WS path /ws/opportunities must not reappear");
mustNot(asyncapi, "/ws/executions", "fictional raw-WS path /ws/executions must not reappear");
mustNot(asyncapi, "/ws/system", "fictional raw-WS path /ws/system must not reappear");

// 4. Corrected core public OpenAPI paths must remain documented.
for (const p of ["/api/opportunities/live", "/api/executions/recent", "/api/readiness", "/api/killswitch/status"]) {
  must(openapi, p, `OpenAPI must retain ${p}`);
}

if (fails.length) {
  console.error("Spec drift check FAILED:");
  for (const f of fails) console.error("  - " + f);
  process.exit(1);
}
console.log("Spec drift assertions passed (" + "leaked-IP, auth-header, raw-WS, core-paths" + ").");
