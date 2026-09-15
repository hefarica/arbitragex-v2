/** Canonical read-only inventory. Registry enablement is NOT broadcast readiness. */
export type CatalogLevel = "chains" | "dexes" | "pools";
export interface CatalogQuery {
  level: CatalogLevel;
  chainId: string | null;
  dexId: string | null;
  after: string | null;
  limit: number;
  search: string;
}
export interface CatalogRow {
  id: string;
  chain_id: string;
  label: string;
  active: boolean | null;
  [key: string]: string | boolean | null;
}
export class CatalogInputError extends Error {}
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const DECIMAL = /^[1-9][0-9]*$/;
const MAX_PG_BIGINT = 9223372036854775807n;

function scalar(query: Record<string, unknown>, key: string): string | null {
  const value = query[key];
  if (value === undefined) return null;
  if (typeof value !== "string") throw new CatalogInputError(`invalid_${key}`);
  return value;
}
export function chainIdentity(value: string): string {
  if (!DECIMAL.test(value) || value.length > 19 || BigInt(value) > MAX_PG_BIGINT) {
    throw new CatalogInputError("invalid_chain_id");
  }
  return value;
}
function uuid(value: string): string {
  if (!UUID.test(value)) throw new CatalogInputError("invalid_uuid");
  return value.toLowerCase();
}
export function parseCatalogQuery(raw: Record<string, unknown>): CatalogQuery {
  const level = scalar(raw, "level") ?? "chains";
  if (level !== "chains" && level !== "dexes" && level !== "pools") {
    throw new CatalogInputError("invalid_level");
  }
  const cid = scalar(raw, "chain_id");
  const did = scalar(raw, "dex_id");
  const after = scalar(raw, "after");
  const limitRaw = scalar(raw, "limit") ?? "25";
  const search = (scalar(raw, "q") ?? "").trim();
  if (!/^[1-9][0-9]{0,2}$/.test(limitRaw) || Number(limitRaw) > 100) {
    throw new CatalogInputError("invalid_limit");
  }
  if (search.length > 100 || /[\u0000-\u001f\u007f]/.test(search)) {
    throw new CatalogInputError("invalid_search");
  }
  if (level === "chains" && (cid !== null || did !== null)) {
    throw new CatalogInputError("chain_list_does_not_accept_scope");
  }
  if (level !== "chains" && cid === null) throw new CatalogInputError("chain_id_required");
  if (level !== "pools" && did !== null) throw new CatalogInputError("dex_scope_requires_pools");
  return {
    level, chainId: cid === null ? null : chainIdentity(cid),
    dexId: did === null ? null : uuid(did),
    after: after === null ? null : level === "chains" ? chainIdentity(after) : uuid(after),
    limit: Number(limitRaw), search,
  };
}

/** Parameters, including the search string and cursor, are NEVER interpolated. */
export function catalogSql(q: CatalogQuery): { text: string; values: unknown[] } {
  const search = `%${q.search.replace(/[\\%_]/g, "\\$&")}%`;
  if (q.level === "chains") return {
    values: [q.after, search, q.limit + 1],
    text: `WITH ids AS (
      SELECT chain_id FROM chains_runtime UNION SELECT chain_id FROM factories WHERE chain_id IS NOT NULL
    ), page AS (
      SELECT i.chain_id, c.name, c.enabled, (c.chain_id IS NOT NULL) AS registered
      FROM ids i LEFT JOIN chains_runtime c ON c.chain_id = i.chain_id
      WHERE ($1::bigint IS NULL OR i.chain_id > $1::bigint)
        AND (COALESCE(c.name, '') ILIKE $2 OR i.chain_id::text ILIKE $2)
      ORDER BY i.chain_id LIMIT $3
    ) SELECT s.chain_id::text AS id, s.chain_id::text AS chain_id,
      COALESCE(s.name, 'Chain ' || s.chain_id::text) AS label,
      s.enabled AS active, s.registered,
      (SELECT count(DISTINCT f.dex_id)::text FROM factories f WHERE f.chain_id=s.chain_id) AS dex_count,
      (SELECT count(*)::text FROM factories f WHERE f.chain_id=s.chain_id) AS factory_count,
      (SELECT count(*)::text FROM pools p JOIN factories f ON f.id=p.factory_id AND f.chain_id=p.chain_id
       JOIN dexes d ON d.id=f.dex_id
       WHERE p.chain_id=s.chain_id) AS pool_count
      FROM page s ORDER BY s.chain_id`,
  };
  if (q.level === "dexes") return {
    values: [q.chainId, q.after, search, q.limit + 1],
    text: `SELECT d.id::text AS id, $1::bigint::text AS chain_id,
      d.name AS label, d.is_active AS active, d.protocol_type,
      (SELECT count(*)::text FROM factories f WHERE f.dex_id=d.id AND f.chain_id=$1) AS factory_count,
      (SELECT count(*)::text FROM pools p JOIN factories f ON f.id=p.factory_id AND f.chain_id=p.chain_id
       WHERE f.dex_id=d.id AND f.chain_id=$1) AS pool_count
      FROM dexes d
      WHERE EXISTS (SELECT 1 FROM factories f WHERE f.dex_id=d.id AND f.chain_id=$1)
        AND ($2::uuid IS NULL OR d.id > $2::uuid)
        AND (d.name ILIKE $3 OR d.protocol_type ILIKE $3)
      ORDER BY d.id LIMIT $4`,
  };
  // The production pool column can be INTEGER while runtime IDs are BIGINT.
  // Type the parameter, not the column: valid large IDs produce an empty page.
  return {
    values: [q.chainId, q.dexId, q.after, search, q.limit + 1],
    text: `SELECT p.id::text AS id, p.chain_id::text AS chain_id, p.address AS label,
      p.is_active AS active, p.address AS pool_address, p.fee_tier::text AS fee_tier,
      d.id::text AS dex_id, d.name AS dex_name, d.is_active AS dex_active, d.protocol_type,
      f.address AS factory_address,
      t0.address AS token0_address, t0.symbol AS token0_symbol,
      t1.address AS token1_address, t1.symbol AS token1_symbol
      FROM pools p JOIN factories f ON f.id=p.factory_id AND f.chain_id=p.chain_id
      JOIN dexes d ON d.id=f.dex_id
      LEFT JOIN tokens t0 ON t0.id=p.token0_id AND t0.chain_id=p.chain_id
      LEFT JOIN tokens t1 ON t1.id=p.token1_id AND t1.chain_id=p.chain_id
      WHERE p.chain_id=$1::bigint AND ($2::uuid IS NULL OR d.id=$2::uuid)
        AND ($3::uuid IS NULL OR p.id > $3::uuid)
        AND (p.address ILIKE $4 OR d.name ILIKE $4 OR d.protocol_type ILIKE $4
          OR t0.address ILIKE $4 OR t1.address ILIKE $4 OR t0.symbol ILIKE $4 OR t1.symbol ILIKE $4)
      ORDER BY p.id LIMIT $5`,
  };
}

const FIELDS: Record<CatalogLevel, readonly string[]> = {
  chains: ["registered", "dex_count", "factory_count", "pool_count"],
  dexes: ["protocol_type", "factory_count", "pool_count"],
  pools: ["pool_address", "fee_tier", "dex_id", "dex_name", "dex_active", "protocol_type",
    "factory_address", "token0_address", "token0_symbol", "token1_address", "token1_symbol"],
};
/** Fail on a schema mismatch, rather than render a made-up zero or ready badge. */
export function catalogPage(q: CatalogQuery, rows: Record<string, unknown>[], observedAt: string) {
  const visible = rows.slice(0, q.limit);
  const items: CatalogRow[] = visible.map((r) => {
    if (typeof r["id"] !== "string" || typeof r["chain_id"] !== "string" ||
        typeof r["label"] !== "string" ||
        (r["active"] !== null && typeof r["active"] !== "boolean")) throw new Error("catalog_schema_mismatch");
    const id = q.level === "chains" ? chainIdentity(r["id"]) : uuid(r["id"]);
    const cid = chainIdentity(r["chain_id"]);
    if ((q.chainId !== null && cid !== q.chainId) || (q.level === "chains" && id !== cid)) {
      throw new Error("catalog_chain_mismatch");
    }
    const item: CatalogRow = { id, chain_id: cid, label: r["label"], active: r["active"] };
    for (const field of FIELDS[q.level]) {
      const value = r[field];
      if (value !== null && typeof value !== "string" && typeof value !== "boolean") {
        throw new Error("catalog_schema_mismatch");
      }
      if (field.endsWith("_count") && (typeof value !== "string" || !/^(0|[1-9][0-9]*)$/.test(value))) {
        throw new Error("catalog_count_invalid");
      }
      if ((field === "registered" && typeof value !== "boolean") ||
          (field === "dex_active" && value !== null && typeof value !== "boolean")) {
        throw new Error("catalog_flag_invalid");
      }
      item[field] = value;
    }
    if (q.dexId !== null && item["dex_id"] !== q.dexId) throw new Error("catalog_dex_mismatch");
    return item;
  });
  return {
    schema_version: 1, source: "postgresql-registry", level: q.level,
    observed_at: observedAt, execution_verified: false,
    scope: { chain_id: q.chainId, dex_id: q.dexId, q: q.search },
    count: items.length, limit: q.limit, items,
    next_after: rows.length > q.limit ? items.at(-1)?.id ?? null : null,
    counts_include_inactive: true,
  };
}
