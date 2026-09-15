/** Validate the complete inventory contract; neither a 200 nor an empty legacy response is success. */
export type CatalogLevel = "chains" | "dexes" | "pools";
export interface CatalogScope { level: CatalogLevel; chainId: string | null; dexId: string | null; search: string }
export interface CatalogItem {
  id: string; chain_id: string; label: string; active: boolean | null;
  [key: string]: string | boolean | null;
}
export interface CatalogResult {
  source: "postgresql-registry"; level: CatalogLevel; observed_at: string;
  items: CatalogItem[]; next_after: string | null; count: number;
}
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const CHAIN = /^[1-9][0-9]{0,18}$/;
function chain(value: unknown): value is string {
  return typeof value === "string" && CHAIN.test(value) && BigInt(value) <= 9223372036854775807n;
}
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Respuesta de catálogo inválida");
  return value as Record<string, unknown>;
}
function follows(id: string, previous: string, level: CatalogLevel): boolean {
  return level === "chains" ? BigInt(id) > BigInt(previous) : id.toLowerCase() > previous.toLowerCase();
}
export function readCatalog(value: unknown, wanted: CatalogScope, after: string | null = null): CatalogResult {
  const data = object(value), scope = object(data["scope"]), limit = data["limit"];
  if (data["schema_version"] !== 1 || data["source"] !== "postgresql-registry" ||
      data["execution_verified"] !== false || data["counts_include_inactive"] !== true ||
      data["level"] !== wanted.level || scope["chain_id"] !== wanted.chainId ||
      scope["dex_id"] !== wanted.dexId || scope["q"] !== wanted.search ||
      typeof limit !== "number" || !Number.isInteger(limit) || limit < 1 || limit > 100 ||
      !Array.isArray(data["items"]) || data["items"].length > limit ||
      data["count"] !== data["items"].length || typeof data["observed_at"] !== "string" ||
      !Number.isFinite(Date.parse(data["observed_at"]))) throw new Error("Contrato de catálogo incompatible");
  let previous = after;
  const items = data["items"].map((value: unknown): CatalogItem => {
    const row = object(value), id = row["id"], cid = row["chain_id"];
    if (typeof id !== "string" || !(wanted.level === "chains" ? chain(id) : UUID.test(id)) ||
        !chain(cid) || typeof row["label"] !== "string" || row["label"].trim() === "" ||
        (row["active"] !== null && typeof row["active"] !== "boolean") ||
        (previous !== null && !follows(id, previous, wanted.level)) ||
        (wanted.chainId !== null && cid !== wanted.chainId) || (wanted.level === "chains" && id !== cid) ||
        (wanted.dexId !== null && row["dex_id"] !== wanted.dexId)) throw new Error("Identidad de catálogo inválida");
    previous = id;
    const counts = wanted.level === "chains" ? ["dex_count", "factory_count", "pool_count"] :
      wanted.level === "dexes" ? ["factory_count", "pool_count"] : [];
    for (const key of counts) {
      const n = row[key];
      if (typeof n !== "string" || !/^(0|[1-9][0-9]*)$/.test(n)) throw new Error("Conteo inválido");
    }
    if (wanted.level === "chains" && typeof row["registered"] !== "boolean") throw new Error("Registro inválido");
    if (wanted.level !== "chains" && (typeof row["protocol_type"] !== "string" || !row["protocol_type"])) {
      throw new Error("Protocolo inválido");
    }
    if (wanted.level === "pools") {
      for (const key of ["pool_address", "dex_id", "dex_name", "factory_address"]) {
        if (typeof row[key] !== "string" || !(row[key] as string).trim()) throw new Error("Identidad de pool incompleta");
      }
      if (!UUID.test(row["dex_id"] as string) || (row["dex_active"] !== null && typeof row["dex_active"] !== "boolean")) {
        throw new Error("Identidad de DEX inválida");
      }
      for (const key of ["fee_tier", "token0_address", "token1_address", "token0_symbol", "token1_symbol"]) {
        if (row[key] !== null && typeof row[key] !== "string") throw new Error("Detalle de pool inválido");
      }
    }
    for (const field of Object.values(row)) {
      if (field !== null && typeof field !== "string" && typeof field !== "boolean") throw new Error("Campo inválido");
    }
    return row as CatalogItem;
  });
  const next = data["next_after"];
  if (next !== null && (typeof next !== "string" || items.length === 0 || next !== items.at(-1)?.id)) {
    throw new Error("Cursor de catálogo inválido");
  }
  return { source: "postgresql-registry", level: wanted.level, observed_at: data["observed_at"],
    count: items.length, items, next_after: next };
}
export function catalogUrl(base: string, scope: CatalogScope, after: string | null): string {
  const query = new URLSearchParams({ view: "liquidity_catalog", level: scope.level, limit: "25" });
  if (scope.chainId !== null) query.set("chain_id", scope.chainId);
  if (scope.dexId !== null) query.set("dex_id", scope.dexId);
  if (scope.search) query.set("q", scope.search);
  if (after !== null) query.set("after", after);
  return `${base.replace(/\/$/, "")}/api/v1/pools?${query}`;
}
