/**
 * ═══════════════════════════════════════════════════════════════════════════════
 * Vector 2: Execution Vector Bridge — FASE OMEGA
 * ═══════════════════════════════════════════════════════════════════════════════
 *
 * Endpoint POST /api/v1/execution que recibe una oportunidad evaluada y retorna
 * 202 Accepted, delegando la ejecución asíncrona al Execution Core.
 *
 * Contrato:
 *   - 202 Accepted: La solicitud de ejecución fue aceptada y encolada.
 *   - 400 Bad Request: Payload inválido o faltan campos requeridos.
 *   - 409 Conflict: La oportunidad ya está en ejecución o fue ejecutada.
 *   - 503 Service Unavailable: Execution Core no disponible.
 *
 * Shadow-safe: En modo shadow/paper, la ejecución es simulada y el resultado
 * se emite al stream de outcomes sin broadcast on-chain.
 */

import type { Application, Request, Response } from "express";
import type { Redis } from "ioredis";
import { randomUUID } from "crypto";

// ── Types ────────────────────────────────────────────────────────────────────

interface ExecutionRequest {
  /** ID de la oportunidad previamente evaluada */
  opportunity_id: string;
  /** Hash de la transacción que generó la oportunidad */
  tx_hash: string;
  /** ID del cartucho que evaluó la oportunidad */
  cartridge_id: string;
  /** Ruta de ejecución (array de pasos) */
  route: ExecutionStep[];
  /** Parámetros de simulación que validaron la oportunidad */
  simulation_params: {
    amount_in: string;
    expected_out: string;
    min_out: string;
    slippage_bps: number;
  };
  /** Metadata del cliente */
  client_meta?: {
    source: string;
    timestamp_ms: number;
  };
}

interface ExecutionStep {
  /** Tipo de paso: swap, flash_loan, approve, etc. */
  step_type: "swap" | "flash_loan" | "approve" | "wrap" | "unwrap";
  /** Protocolo/DEX objetivo */
  protocol: string;
  /** Dirección del pool o contrato */
  target: string;
  /** Token de entrada */
  token_in: string;
  /** Token de salida */
  token_out: string;
  /** Monto (en wei/unidades base) */
  amount: string;
  /** Datos adicionales específicos del paso */
  data?: Record<string, unknown>;
}

interface ExecutionResponse {
  /** ID único de la solicitud de ejecución */
  execution_id: string;
  /** Estado inicial de la ejecución */
  status: "queued" | "simulating" | "rejected";
  /** Timestamp de aceptación */
  accepted_at: string;
  /** Estimación de inicio (ms desde ahora) */
  estimated_start_ms: number;
  /** Mensaje descriptivo */
  message: string;
}

interface ExecutionError {
  error: string;
  code: string;
  details?: Record<string, unknown>;
}

// ── Constants ─────────────────────────────────────────────────────────────────

const EXECUTION_STREAM = "arbx:execution:requests";
const EXECUTION_MAXLEN = 100_000;
const DEFAULT_ESTIMATED_START_MS = 500;

// ── Validation ────────────────────────────────────────────────────────────────

function validateExecutionRequest(body: unknown): { valid: true; data: ExecutionRequest } | { valid: false; error: string } {
  if (typeof body !== "object" || body === null) {
    return { valid: false, error: "Request body must be an object" };
  }

  const req = body as Partial<ExecutionRequest>;

  if (!req.opportunity_id || typeof req.opportunity_id !== "string") {
    return { valid: false, error: "Missing or invalid opportunity_id" };
  }

  if (!req.tx_hash || typeof req.tx_hash !== "string") {
    return { valid: false, error: "Missing or invalid tx_hash" };
  }

  if (!req.cartridge_id || typeof req.cartridge_id !== "string") {
    return { valid: false, error: "Missing or invalid cartridge_id" };
  }

  if (!Array.isArray(req.route) || req.route.length === 0) {
    return { valid: false, error: "Missing or invalid route (must be non-empty array)" };
  }

  for (const step of req.route) {
    if (!step.step_type || !["swap", "flash_loan", "approve", "wrap", "unwrap"].includes(step.step_type)) {
      return { valid: false, error: `Invalid step_type: ${step.step_type}` };
    }
    if (!step.protocol || typeof step.protocol !== "string") {
      return { valid: false, error: "Missing or invalid protocol in route step" };
    }
    if (!step.target || typeof step.target !== "string") {
      return { valid: false, error: "Missing or invalid target in route step" };
    }
    if (!step.token_in || typeof step.token_in !== "string") {
      return { valid: false, error: "Missing or invalid token_in in route step" };
    }
    if (!step.token_out || typeof step.token_out !== "string") {
      return { valid: false, error: "Missing or invalid token_out in route step" };
    }
    if (!step.amount || typeof step.amount !== "string") {
      return { valid: false, error: "Missing or invalid amount in route step" };
    }
  }

  if (!req.simulation_params || typeof req.simulation_params !== "object") {
    return { valid: false, error: "Missing or invalid simulation_params" };
  }

  const sim = req.simulation_params as Partial<ExecutionRequest["simulation_params"]>;
  if (!sim.amount_in || typeof sim.amount_in !== "string") {
    return { valid: false, error: "Missing or invalid simulation_params.amount_in" };
  }
  if (!sim.expected_out || typeof sim.expected_out !== "string") {
    return { valid: false, error: "Missing or invalid simulation_params.expected_out" };
  }
  if (!sim.min_out || typeof sim.min_out !== "string") {
    return { valid: false, error: "Missing or invalid simulation_params.min_out" };
  }
  if (typeof sim.slippage_bps !== "number" || sim.slippage_bps < 0 || sim.slippage_bps > 10000) {
    return { valid: false, error: "Invalid simulation_params.slippage_bps (must be 0-10000)" };
  }

  return { valid: true, data: req as ExecutionRequest };
}

// ── Route Mount Function ─────────────────────────────────────────────────────

export function mountExecution(app: Application, deps: { redis: Redis }): void {
  app.post("/api/v1/execution", async (req: Request, res: Response) => {
    const validation = validateExecutionRequest(req.body);

    if (!validation.valid) {
      const error: ExecutionError = {
        error: validation.error,
        code: "VALIDATION_FAILED",
      };
      return res.status(400).json(error);
    }

    const execReq = validation.data;
    const executionId = randomUUID();
    const acceptedAt = new Date().toISOString();

    // Verificar si la oportunidad ya está en ejecución (Redis dedup)
    const dedupKey = `arbx:execution:dedup:${execReq.opportunity_id}`;
    try {
      const existing = await deps.redis.get(dedupKey);
      if (existing) {
        const error: ExecutionError = {
          error: "Opportunity already in execution or executed",
          code: "ALREADY_EXECUTING",
          details: {
            opportunity_id: execReq.opportunity_id,
            existing_execution_id: existing,
          },
        };
        return res.status(409).json(error);
      }

      // Set dedup key con TTL de 5 minutos
      await deps.redis.setex(dedupKey, 300, executionId);
    } catch (e) {
      console.error("[execution] Redis dedup check failed:", e);
      const error: ExecutionError = {
        error: "Execution service unavailable",
        code: "REDIS_UNAVAILABLE",
      };
      return res.status(503).json(error);
    }

    // Construir payload para el Execution Core
    const executionPayload = {
      execution_id: executionId,
      opportunity_id: execReq.opportunity_id,
      tx_hash: execReq.tx_hash,
      cartridge_id: execReq.cartridge_id,
      route: execReq.route,
      simulation_params: execReq.simulation_params,
      client_meta: execReq.client_meta ?? {
        source: "api",
        timestamp_ms: Date.now(),
      },
      accepted_at: acceptedAt,
    };

    // Encolar en Redis Stream para el Execution Core
    try {
      await deps.redis.xadd(
        EXECUTION_STREAM,
        "MAXLEN",
        "~",
        EXECUTION_MAXLEN,
        "*",
        "json",
        JSON.stringify(executionPayload)
      );
    } catch (e) {
      console.error("[execution] Failed to queue execution:", e);
      // Limpiar dedup key si falla el encolado
      await deps.redis.del(dedupKey).catch(() => {});
      const error: ExecutionError = {
        error: "Failed to queue execution",
        code: "QUEUE_FAILED",
      };
      return res.status(503).json(error);
    }

    // Responder 202 Accepted
    const response: ExecutionResponse = {
      execution_id: executionId,
      status: "queued",
      accepted_at: acceptedAt,
      estimated_start_ms: DEFAULT_ESTIMATED_START_MS,
      message: "Execution request accepted and queued for processing",
    };

    res.status(202).json(response);
  });

  // Endpoint GET para consultar estado de ejecución
  app.get("/api/v1/execution/:executionId", async (req: Request, res: Response) => {
    const { executionId } = req.params;

    if (!executionId || typeof executionId !== "string") {
      const error: ExecutionError = {
        error: "Invalid execution_id",
        code: "INVALID_ID",
      };
      return res.status(400).json(error);
    }

    // Buscar en Redis el estado de la ejecución
    const statusKey = `arbx:execution:status:${executionId}`;
    try {
      const status = await deps.redis.get(statusKey);
      if (!status) {
        const error: ExecutionError = {
          error: "Execution not found",
          code: "NOT_FOUND",
        };
        return res.status(404).json(error);
      }

      const parsed = JSON.parse(status);
      return res.json({
        execution_id: executionId,
        ...parsed,
      });
    } catch (e) {
      console.error("[execution] Failed to get execution status:", e);
      const error: ExecutionError = {
        error: "Execution service unavailable",
        code: "REDIS_UNAVAILABLE",
      };
      return res.status(503).json(error);
    }
  });
}
