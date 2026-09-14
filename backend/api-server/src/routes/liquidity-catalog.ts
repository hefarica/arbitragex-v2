import { CatalogInputError, catalogPage, catalogSql, parseCatalogQuery } from "./liquidity-catalog-core.js";

// Structural contracts keep the reader independent of Express and PostgreSQL drivers.
// Production receives the SAME pool and request/response used by mountPools.
interface CatalogConnection {
  query(text: string, values?: unknown[]): Promise<{ rows: Record<string, unknown>[] }>;
  release(error?: Error): void;
}
interface CatalogDependencies {
  pool: { connect(): Promise<CatalogConnection> } | null;
  logger: { warn(obj: object, message?: string): void };
}
interface CatalogResponse {
  status(code: number): CatalogResponse;
  json(body: unknown): unknown;
  setHeader(name: string, value: string): unknown;
}
/** An expired acquisition must release a connection that arrives late. */
export function acquireCatalogConnection(
  pool: NonNullable<CatalogDependencies["pool"]>, timeoutMs = 1000,
): Promise<CatalogConnection> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => {
      settled = true;
      reject(new Error("catalog_acquire_timeout"));
    }, timeoutMs);
    void pool.connect().then((client) => {
      if (settled) { client.release(); return; }
      settled = true; clearTimeout(timer); resolve(client);
    }, () => {
      if (settled) return;
      settled = true; clearTimeout(timer); reject(new Error("catalog_connect_failed"));
    });
  });
}
export async function serveLiquidityCatalog(
  req: { query: Record<string, unknown> }, res: CatalogResponse, deps: CatalogDependencies,
): Promise<void> {
  // A cached registry response remains a timestamped inventory, not chain state.
  res.setHeader("Cache-Control", "no-store");
  let query;
  try { query = parseCatalogQuery(req.query); }
  catch (error) {
    if (!(error instanceof CatalogInputError)) throw error;
    res.status(400).json({ error: error.message });
    return;
  }
  if (!deps.pool) { res.status(503).json({ error: "db_unavailable" }); return; }
  let client: CatalogConnection | null = null;
  let inTransaction = false;
  let discard: Error | undefined;
  try {
    client = await acquireCatalogConnection(deps.pool);
    await client.query("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY");
    inTransaction = true;
    await client.query("SET LOCAL statement_timeout = '4000ms'");
    await client.query("SET LOCAL lock_timeout = '1000ms'");
    const sql = catalogSql(query);
    const result = await client.query(sql.text, sql.values);
    const page = catalogPage(query, result.rows, new Date().toISOString());
    await client.query("COMMIT");
    inTransaction = false;
    res.status(200).json(page);
  } catch {
    if (client && inTransaction) {
      try { await client.query("ROLLBACK"); }
      catch { discard = new Error("catalog_connection_discarded"); }
    }
    // Never return driver errors, SQL, connection strings, or RPC credentials.
    deps.logger.warn({ event: "liquidity_catalog.unavailable", level: query.level });
    res.status(503).json({ error: "catalog_unavailable" });
  } finally {
    if (client) { if (discard) client.release(discard); else client.release(); }
  }
}
