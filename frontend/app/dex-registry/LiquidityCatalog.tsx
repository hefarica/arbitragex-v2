"use client";

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import { catalogUrl, readCatalog, type CatalogScope, type CatalogResult, type CatalogItem } from "./liquidity-catalog-model";

const ROOT: CatalogScope = { level: "chains", chainId: null, dexId: null, search: "" };
const button = "rounded-lg border border-white/15 px-3 py-2 text-sm hover:bg-white/10 disabled:opacity-40 disabled:cursor-not-allowed";
function value(row: CatalogItem, key: string): string {
  const item = row[key];
  return typeof item === "string" ? item : "—";
}

/** Extends /dex-registry. All reads go through the existing Edge /api/v1/pools contract. */
export default function LiquidityCatalog() {
  const [scope, setScope] = useState<CatalogScope>(ROOT);
  const [after, setAfter] = useState<string | null>(null);
  const [history, setHistory] = useState<(string | null)[]>([]);
  const [draft, setDraft] = useState("");
  const [refresh, setRefresh] = useState(0);
  const [data, setData] = useState<CatalogResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(true);

  useEffect(() => {
    let cancelled = false;
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 8000);
    setBusy(true); setError(null); setData(null);
    void (async () => {
      try {
        const response = await fetch(catalogUrl(getApiBaseUrl(), scope, after), {
          signal: controller.signal, headers: { accept: "application/json" }, cache: "no-store",
        });
        if (!response.ok) throw new Error(`Catálogo no disponible (HTTP ${response.status})`);
        const result = readCatalog(await response.json(), scope, after);
        if (!cancelled) setData(result);
      } catch (failure) {
        if (!cancelled) setError(controller.signal.aborted
          ? "La consulta superó el tiempo permitido. No se muestran datos anteriores."
          : failure instanceof Error ? failure.message : "No se pudo consultar el catálogo");
      } finally {
        clearTimeout(timeout);
        if (!cancelled) setBusy(false);
      }
    })();
    return () => { cancelled = true; clearTimeout(timeout); controller.abort(); };
  }, [scope, after, refresh]);

  function navigate(next: CatalogScope) {
    setScope({ ...next }); setAfter(null); setHistory([]); setDraft(next.search);
    // Remove the old scope's rows immediately, before the next effect.
    setData(null); setError(null); setBusy(true);
  }
  function drill(row: CatalogItem) {
    if (scope.level === "chains") navigate({ level: "dexes", chainId: row.chain_id, dexId: null, search: "" });
    else if (scope.level === "dexes") navigate({ level: "pools", chainId: row.chain_id, dexId: row.id, search: "" });
  }

  return (
    <section aria-labelledby="liquidity-catalog-title" data-testid="liquidity-catalog"
      className="mx-auto mb-6 max-w-7xl rounded-2xl border border-white/15 bg-white/5 p-5 text-white">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <p className="text-xs uppercase tracking-widest text-slate-400">Inventario multichain · fuente canónica</p>
          <h2 id="liquidity-catalog-title" className="mt-1 text-xl font-semibold">Blockchains → DEX → Pools</h2>
          <p className="mt-2 max-w-3xl text-sm text-slate-300">
            PostgreSQL: cadenas configuradas y factories registradas. Los conteos incluyen registros activos e inactivos.
            Un registro activo no acredita RPC saludable, liquidez vigente ni autorización para operar.
          </p>
        </div>
        <button type="button" className={button} disabled={busy} onClick={() => setRefresh((v) => v + 1)}>Actualizar catálogo</button>
      </div>
      <nav aria-label="Administración de liquidez" className="mt-4 flex flex-wrap gap-3 text-sm text-sky-300">
        <a className="underline" href="/admin/chains">Agregar o configurar blockchain</a>
        <a className="underline" href="#dex-admin">Agregar o administrar DEX</a>
        <a className="underline" href="/pools">Panel de pools</a>
        <a className="underline" href="/live-readiness">Verificación de ejecución LIVE</a>
      </nav>
      <p className="mt-3 rounded-lg border border-amber-300/20 p-3 text-sm text-amber-100">
        Testnet LIVE y Mainnet LIVE: este catálogo no activa firmantes ni envía transacciones.
        La ejecución exige evidencia independiente por cadena y protocolo; este inventario no la certifica.
      </p>
      <nav aria-label="Nivel del agregador" className="mt-4 flex flex-wrap gap-2">
        <button type="button" className={button} onClick={() => navigate(ROOT)}>Blockchains</button>
        {scope.chainId !== null && <button type="button" className={button}
          onClick={() => navigate({ level: "dexes", chainId: scope.chainId, dexId: null, search: "" })}>Chain {scope.chainId} · DEX</button>}
        {scope.chainId !== null && <button type="button" className={button}
          onClick={() => navigate({ level: "pools", chainId: scope.chainId, dexId: null, search: "" })}>Todos sus pools</button>}
      </nav>
      <form className="mt-4 flex gap-2" onSubmit={(event) => {
        event.preventDefault(); navigate({ ...scope, search: draft.trim() });
      }}>
        <label htmlFor="catalog-search" className="sr-only">Buscar en el nivel actual</label>
        <input id="catalog-search" value={draft} maxLength={100} onChange={(event) => setDraft(event.target.value)}
          placeholder={scope.level === "pools" ? "Dirección, símbolo, DEX o protocolo" : "Nombre, cadena o protocolo"}
          className="min-w-0 flex-1 rounded-lg border border-white/20 bg-transparent px-3 py-2 text-sm" />
        <button className={button} type="submit">Buscar</button>
      </form>
      {scope.dexId !== null && <p className="mt-3 break-all text-xs text-slate-400">DEX seleccionado: {scope.dexId}</p>}
      <div aria-live="polite" className="mt-4">
        {busy && <p role="status" className="text-slate-300">Consultando el registro…</p>}
        {error && <p role="alert" className="rounded-lg border border-red-300/30 p-3 text-red-200">{error}</p>}
        {data && <>
          <p className="mb-3 text-xs text-slate-400">Fuente: {data.source} · Captura API: {data.observed_at} · {data.count} filas en esta página</p>
          {data.items.length === 0 ? <p>No hay registros que coincidan con esta consulta.</p> :
            <div className="overflow-x-auto">
              <table className="w-full text-left text-sm">
                <caption className="sr-only">Inventario {scope.level} por cadena, no evidencia de ejecución</caption>
                <thead><tr className="border-b border-white/15 text-slate-400">
                  <th scope="col" className="p-2">{scope.level === "chains" ? "Blockchain" : scope.level === "dexes" ? "DEX" : "Pool"}</th>
                  <th scope="col" className="p-2">Registro</th><th scope="col" className="p-2">Detalle</th>
                  <th scope="col" className="p-2">{scope.level === "pools" ? "Identidad" : "Explorar"}</th>
                </tr></thead>
                <tbody>{data.items.map((row) => <tr key={`${row.chain_id}:${row.id}`} data-testid="catalog-row" data-id={row.id} data-chain-id={row.chain_id} data-dex-id={typeof row["dex_id"] === "string" ? row["dex_id"] : undefined} className="border-b border-white/10 align-top">
                  <td className="max-w-xs break-all p-2"><span className="font-medium">{row.label}</span>
                    <div className="text-xs text-slate-400">Chain ID: {row.chain_id}</div>
                    {scope.level === "pools" && <div className="mt-1 text-xs">{value(row, "token0_symbol")} / {value(row, "token1_symbol")}</div>}
                  </td>
                  <td className="p-2">{row.active === true ? "Activo" : row.active === false ? "Inactivo" : "Sin configuración runtime"}
                    {scope.level === "chains" && row["registered"] === false && <div className="text-xs text-amber-200">Solo factories registradas</div>}
                    {scope.level === "pools" && row["dex_active"] === false && <div className="text-xs text-amber-200">DEX inactivo</div>}
                  </td>
                  <td className="p-2 text-slate-300">
                    {scope.level === "chains" ? <>{value(row, "dex_count")} DEX · {value(row, "factory_count")} factories · {value(row, "pool_count")} pools</> :
                      scope.level === "dexes" ? <>{value(row, "protocol_type")}<br />{value(row, "factory_count")} factories · {value(row, "pool_count")} pools</> :
                      <>{value(row, "dex_name")} · {value(row, "protocol_type")}<br />Fee tier (registro): {value(row, "fee_tier")}</>}
                  </td>
                  <td className="max-w-sm break-all p-2">
                    {scope.level !== "pools" ? <button type="button" className={button} onClick={() => drill(row)}>
                      {scope.level === "chains" ? "Ver DEX" : "Ver pools"}</button> : <details>
                      <summary className="cursor-pointer text-sky-300">Direcciones completas</summary>
                      <p className="mt-2 text-xs">Factory: {value(row, "factory_address")}</p>
                      <p className="mt-1 text-xs">Token 0: {value(row, "token0_address")}</p>
                      <p className="mt-1 text-xs">Token 1: {value(row, "token1_address")}</p>
                      <p className="mt-1 text-xs">Pool ID: {row.id}</p>
                    </details>}
                  </td>
                </tr>)}</tbody>
              </table>
            </div>}
          <div className="mt-4 flex items-center gap-3">
            <button type="button" className={button} disabled={history.length === 0} onClick={() => {
              setAfter(history.at(-1) ?? null); setHistory((pages) => pages.slice(0, -1)); setData(null); setBusy(true);
            }}>Anterior</button>
            <span className="text-xs text-slate-400">Página {history.length + 1}</span>
            <button type="button" className={button} disabled={data.next_after === null} onClick={() => {
              setHistory((pages) => [...pages, after]); setAfter(data.next_after); setData(null); setBusy(true);
            }}>Siguiente</button>
          </div>
        </>}
      </div>
    </section>
  );
}
