/** Portable ARBX-CJSON-1 / receipt adapter. No endpoints, keys or state toggles.
 * Import explicitly at real service boundaries; never infer backend ACKs from UI.
 * Decimal quantities are strings. This restricted profile is not general JCS.
 */
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export interface Receipt {
  schema: "arbx.receipt.v1";
  event_id: string;
  stage: string;
  context_id: string;
  at_ms: number;
  transform_id: string;
  parent_hash: string | null;
  input_hash: string | null;
  output_hash: string;
  payload: Json;
  receipt_hash: string;
}
const hashPattern = /^[0-9a-f]{64}$/;

export function canonicalJson(value: Json): string {
  let nodes = 0;
  const walk = (v: Json, depth: number): string => {
    if (++nodes > 100000 || depth > 32) throw new Error("payload complexity limit");
    if (v === null || typeof v === "boolean") return JSON.stringify(v);
    if (typeof v === "number") {
      if (!Number.isSafeInteger(v)) throw new Error("decimal/large values require strings");
      return String(v);
    }
    if (typeof v === "string") {
      // Reject lone UTF-16 surrogates to match Python's strict UTF-8 profile.
      if (/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u.test(v)) {
        throw new Error("unpaired surrogate");
      }
      return JSON.stringify(v);
    }
    if (Array.isArray(v)) {
      for (let i = 0; i < v.length; i++) {
        if (!Object.hasOwn(v, i)) throw new Error("sparse arrays refused");
      }
      return "[" + v.map((x) => walk(x, depth + 1)).join(",") + "]";
    }
    if (typeof v !== "object" || v === undefined) throw new Error("unsupported JSON value");
    const proto = Object.getPrototypeOf(v);
    if (proto !== null && proto !== Object.prototype) throw new Error("plain JSON object required");
    if (Object.getOwnPropertySymbols(v).length) throw new Error("symbol key not permitted");
    return "{" + Object.keys(v).sort().map((key) => {
      if (!/^[\x00-\x7F]*$/.test(key) || key.length > 256) throw new Error("ASCII key required");
      return JSON.stringify(key) + ":" + walk(v[key], depth + 1);
    }).join(",") + "}";
  };
  return walk(value, 0);
}

export async function sha256(value: Json): Promise<string> {
  const bytes = new TextEncoder().encode(canonicalJson(value));
  const result = await globalThis.crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(result), (b) => b.toString(16).padStart(2, "0")).join("");
}

export async function makeReceipt(
  eventId: string, stage: string, contextId: string, payload: Json,
  atMs: number, parent: Receipt | null = null, transformId = "identity.v1",
): Promise<Receipt> {
  if (!eventId || !stage || !hashPattern.test(contextId)) throw new Error("identity missing");
  if (!Number.isSafeInteger(atMs) || atMs <= 0) throw new Error("timestamp invalid");
  if (parent && (parent.event_id !== eventId || parent.context_id !== contextId)) {
    throw new Error("cross-opportunity evidence refused");
  }
  const body = {
    schema: "arbx.receipt.v1" as const, event_id: eventId, stage, context_id: contextId,
    at_ms: atMs, transform_id: transformId, parent_hash: parent?.receipt_hash ?? null,
    input_hash: parent?.output_hash ?? null, output_hash: await sha256(payload), payload,
  };
  return { ...body, receipt_hash: await sha256(body) };
}

export interface FieldDiff { field: string; issue: "dropped" | "nullified" | "invented" | "changed" }
/** Field preservation only; aliases and allowed transforms must be explicit. */
export function compareFields(before: Record<string, Json>, after: Record<string, Json>,
  fields: readonly string[], aliases: Readonly<Record<string, string>> = {}): FieldDiff[] {
  const errors: FieldDiff[] = [];
  for (const field of fields) {
    const target = aliases[field] ?? field;
    const hasBefore = Object.hasOwn(before, field), hasAfter = Object.hasOwn(after, target);
    if (hasBefore && !hasAfter) errors.push({field, issue: "dropped"});
    else if ((!hasBefore || before[field] === null) && hasAfter && after[target] !== null) errors.push({field, issue: "invented"});
    else if (hasBefore && hasAfter && before[field] !== null && after[target] === null) errors.push({field, issue: "nullified"});
    else if (hasBefore && hasAfter && canonicalJson(before[field]) !== canonicalJson(after[target])) errors.push({field, issue: "changed"});
  }
  return errors;
}

export interface CardObservation {
  event_id: string; observed_at_ms: number; in_dom: boolean; has_layout_box: boolean;
  fields: {label: string; text: string}[]; scope: "dom_text_observation_not_backend_ack";
}
/** Call after the card commit. Does not claim the user saw/read these pixels. */
export function observeCard(eventId: string, root: ParentNode = document): CardObservation {
  const card = Array.from(root.querySelectorAll<HTMLElement>("[data-opp-id]"))
    .find((el) => el.dataset.oppId === eventId);
  const rect = card?.getBoundingClientRect();
  const cells = card?.querySelectorAll<HTMLElement>('[data-testid="opportunity-summary-grid"] .min-w-0');
  return { event_id: eventId, observed_at_ms: Date.now(), in_dom: !!card,
    has_layout_box: !!rect && rect.width > 0 && rect.height > 0,
    fields: cells ? Array.from(cells).map((c) => ({
      label: c.children[0]?.textContent?.trim() ?? "", text: c.children[1]?.textContent?.trim() ?? "",
    })) : [], scope: "dom_text_observation_not_backend_ack" };
}
