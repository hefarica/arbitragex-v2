/**
 * POOL-CIRCULARITY-01 (C2) — barrido periódico de veredictos de seguridad de token.
 *
 * POR QUÉ EXISTE ESTE ARCHIVO
 * ---------------------------
 * `checkToken` se llamaba en EXACTAMENTE dos sitios, y los dos dentro del paso de
 * candidato (`consumer.ts:411-412`). Esa es la mitad de una dependencia circular:
 *
 *     sin candidatos -> no se llama checkToken -> token_safety_cache CONGELADO
 *     sin veredictos frescos -> 0 pools reactivables (pool_enumeration_worker.rs:496-511
 *                               exige ttl_expires_at > NOW())
 *     sin reactivación -> el universo no crece -> todo llega YA rechazado
 *     -> el mensaje se rechaza sin evaluar -> sin candidatos
 *
 * Este barrido rompe el círculo por el lado de los VEREDICTOS: produce
 * veredictos frescos sobre el catálogo **sin depender de que entre un candidato**.
 *
 * QUÉ NO HACE (línea roja, explícita)
 * ----------------------------------
 * - **NO saltea el gate de seguridad.** LLAMA a `checkToken`, no lo evita. Cada
 *   veredicto sale del mismo camino real: canónico → cache → proveedor (GoPlus,
 *   bajo CircuitBreaker) → heurística interna. `TOKEN_SAFETY_FLOOR` y
 *   `min_acceptable_score` quedan intactos.
 * - **NO fabrica filas.** La única escritura la hace `checkToken`/`upsertCached`
 *   dentro del camino real. Este módulo no emite `INSERT`/`UPDATE` propios.
 * - **NO toca umbrales** de spread ni de net-profit: no los conoce ni los lee.
 * - **NO decide nada.** No activa pools, no rechaza oportunidades, no publica.
 *   Sólo deja veredictos frescos en la cache para que los lectores aguas abajo
 *   (activación de pool-enumeration, pre-execute checklist) tengan algo que leer.
 *
 * El efecto aguas abajo es **deliberadamente indirecto**: recuperar veredictos
 * devuelve oportunidades EVALUABLES, no rentables. El motor puede volver a
 * rechazarlas por economía, y eso está bien.
 */

import type pg from "pg";
import type { Logger } from "pino";
import type { AppConfig, CircuitBreaker } from "@arbx/shared";
import { checkToken } from "./client.js";

/** Tope de tokens por corrida: acota el coste del barrido y deja respirar al proveedor. */
const DEFAULT_BATCH_LIMIT = 500;
/** Intervalo por defecto entre barridos (15 min). Configurable por entorno. */
const DEFAULT_INTERVAL_MS = 15 * 60 * 1000;
/** Retardo del primer barrido tras el arranque: no competir con el warm-up. */
const DEFAULT_FIRST_DELAY_MS = 30_000;

export interface SweepDeps {
  pool: pg.Pool;
  cb: CircuitBreaker;
  cfg: AppConfig;
  logger: Logger;
}

export interface SweepResult {
  /** Tokens del catálogo considerados en esta corrida. */
  considered: number;
  /** Veredictos producidos (todos pasan por el gate real). */
  verdicts: number;
  /** Tokens que fallaron; el barrido continúa con el resto. */
  failures: number;
}

/**
 * Tokens del catálogo = los que los pools ya referencian (`pools.token0_id` /
 * `token1_id` son FK a `tokens.id`). Medido en el VPS: 3.453 tokens distintos.
 *
 * Cadena con UN solo pool incluido a propósito: 617 de los 804 pares tienen un
 * único pool, y son justamente los que no pueden formar ciclo hasta que el
 * universo crezca.
 */
export async function listCatalogTokens(
  pool: pg.Pool,
  limit: number,
): Promise<Array<{ chain_id: number; address: string }>> {
  const { rows } = await pool.query<{ chain_id: number; address: string }>(
    `SELECT DISTINCT t.chain_id, t.address
       FROM tokens t
      WHERE t.id IN (SELECT token0_id FROM pools UNION SELECT token1_id FROM pools)
      ORDER BY t.chain_id, t.address
      LIMIT $1`,
    [limit],
  );
  return rows;
}

/**
 * Una pasada del barrido. Fail-honest: un token que falla NO aborta la corrida —
 * se cuenta y se sigue. Devuelve los conteos reales, nunca una estimación.
 */
export async function sweepCatalogTokens(
  deps: SweepDeps,
  limit: number = DEFAULT_BATCH_LIMIT,
): Promise<SweepResult> {
  const { pool, cb, cfg, logger } = deps;
  const started = Date.now();

  const tokens = await listCatalogTokens(pool, limit);
  let verdicts = 0;
  let failures = 0;

  for (const t of tokens) {
    try {
      // LLAMA al gate real. No hay atajo, no hay valor por defecto, no hay fila
      // pre-hecha: si el gate no puede dictaminar, `checkToken` persiste el
      // resultado de la heurística interna con source='internal', que es una
      // respuesta honesta del gate, no una invención de este módulo.
      await checkToken(pool, cb, cfg, t.chain_id, t.address);
      verdicts++;
    } catch (err) {
      failures++;
      logger.warn({
        event: "token_safety.sweep_token_failed",
        chain_id: t.chain_id,
        token: t.address,
        error: (err as Error).message,
      });
    }
  }

  const result: SweepResult = { considered: tokens.length, verdicts, failures };
  logger.info({
    event: "token_safety.sweep_done",
    ...result,
    duration_ms: Date.now() - started,
  });
  return result;
}

/**
 * Arranca el barrido periódico. Devuelve una función de parada.
 *
 * Intervalo: `ARBX_TOKEN_SAFETY_SWEEP_SECS` (por defecto 900 s = 15 min).
 * `0` desactiva el barrido (kill-switch de operador, sin tocar código).
 */
export function startTokenSafetySweep(deps: SweepDeps): () => void {
  const raw = process.env["ARBX_TOKEN_SAFETY_SWEEP_SECS"];
  const parsed = raw === undefined ? NaN : Number(raw);
  const intervalMs = Number.isFinite(parsed) && parsed >= 0 ? parsed * 1000 : DEFAULT_INTERVAL_MS;

  if (intervalMs === 0) {
    deps.logger.info({
      event: "token_safety.sweep_disabled",
      reason: "ARBX_TOKEN_SAFETY_SWEEP_SECS=0",
    });
    return () => {};
  }

  let running = false;
  const tick = async (): Promise<void> => {
    // No solapar corridas: un barrido lento no debe apilarse sobre sí mismo.
    if (running) {
      deps.logger.warn({ event: "token_safety.sweep_skipped", reason: "previous_run_in_flight" });
      return;
    }
    running = true;
    try {
      await sweepCatalogTokens(deps);
    } catch (err) {
      deps.logger.error({ event: "token_safety.sweep_failed", error: (err as Error).message });
    } finally {
      running = false;
    }
  };

  const firstTimer = setTimeout(() => void tick(), DEFAULT_FIRST_DELAY_MS);
  const timer = setInterval(() => void tick(), intervalMs);
  // No mantener vivo el proceso sólo por este temporizador.
  if (typeof firstTimer.unref === "function") firstTimer.unref();
  if (typeof timer.unref === "function") timer.unref();

  deps.logger.info({ event: "token_safety.sweep_started", interval_ms: intervalMs });

  return () => {
    clearTimeout(firstTimer);
    clearInterval(timer);
    deps.logger.info({ event: "token_safety.sweep_stopped" });
  };
}
