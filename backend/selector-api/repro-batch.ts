// Corre el parser REAL sobre 120 payloads REALES capturados del stream vivo.
import { readFileSync } from "node:fs";
import { SelectorOpportunitySchema } from "./src/consumer.js";
import { OpportunitySchema } from "@arbx/shared";

const raw = readFileSync("../../live-payloads.json", "utf8").replace(/^\uFEFF/, "");
const entries: [string, string[]][] = JSON.parse(raw);

const payloads = entries
  .map(([, kv]) => { const i = kv.indexOf("json"); return i >= 0 ? kv[i + 1] : undefined; })
  .filter((s): s is string => !!s)
  .map((s) => { try { return JSON.parse(s); } catch { return null; } })
  .filter((o): o is any => !!o);

let beforeOk = 0, afterOk = 0;
const keyHist = new Map<string, number>();
for (const p of payloads) {
  if (OpportunitySchema.safeParse(p).success) beforeOk++;
  const r = SelectorOpportunitySchema.safeParse(p);
  if (r.success) afterOk++;
  else for (const i of r.error.issues) {
    const k = i.code === "unrecognized_keys" ? JSON.stringify((i as any).keys) : i.code;
    keyHist.set(k, (keyHist.get(k) ?? 0) + 1);
  }
}

console.log("payloads reales parseados:", payloads.length);
console.log("ANTES  (OpportunitySchema .strict())          :", beforeOk, "/", payloads.length, "PASAN");
console.log("DESPUES(SelectorOpportunitySchema, el real)   :", afterOk, "/", payloads.length, "PASAN");
if (keyHist.size) { console.log("fallos residuales del fix:"); for (const [k, v] of keyHist) console.log("   ", k, "x", v); }
else console.log("fallos residuales del fix: (ninguno)");
