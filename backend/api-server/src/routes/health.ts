/**
 * =============================================================================
 * OMEGA Health & Telemetry — Observability Canónica del Sistema
 * =============================================================================
 *
 * Endpoints:
 *   GET  /health              — Estado holístico del sistema OMEGA
 *   GET  /metrics/entropy     — Entropía termodinámica actual
 *   WS   /ws/metrics          — Streaming de métricas en tiempo real
 *
 * Léxico OMEGA (LEY DE LEXICÓN ABSOLUTO):
 *   - Entropía: Medida de dispersión del estado topológico
 *   - Convergencia: Tendencia hacia el equilibrio ortogonal
 *   - Topología: Estructura de la red de variedades de liquidez
 *
 * @module routes/health
 */

import { Router, type Request, type Response, type RequestHandler } from "express";
import type { Pool } from "pg";
import type { Redis } from "ioredis";
import type { Server as IoServer } from "socket.io";

/** Estado de un servicio del ecosistema OMEGA */
interface ServiceStatus {
  status: "running" | "degraded" | "stopped" | "unknown";
  latency_ms?: number;
  last_check: string;
  detail?: string;
}

/** Respuesta canónica del endpoint /health */
interface HealthResponse {
  system_status: "healthy" | "degraded" | "critical";
  math_guardian: "passed" | "warning" | "failed";
  entropy: number;
  timestamp: number;
  chains: string[];
  services: Record<string, ServiceStatus>;
  convergence: {
    rate: number;
    target: number;
    variance: number;
  };
  topology: {
    manifolds_observed: number;
    loops_resolved: number;
    decoherence_rate: number;
  };
}

/** Respuesta del endpoint /metrics/entropy
 *
 * FAIL-HONEST: entropy es null cuando no hay datos suficientes para calcular
 * una métrica significativa (R8 — Doctrina OMEGA).
 */
interface EntropyResponse {
  entropy: number | null;
  delta: number;
  timestamp: number;
  window_seconds: number;
  /** Indica si hay datos suficientes para el cálculo */
  has_data: boolean;
}

/** Dependencias requeridas para el módulo de salud */
export interface HealthDeps {
  pool: Pool | null;
  redis: Redis;
  io: IoServer;
}

/** Configuración de cadenas observadas por el sistema OMEGA */
const OBSERVED_CHAINS = ["ETH", "ARB", "BASE", "POLY", "OP"];

/** URLs de servicios upstream para verificación de salud */
const UPSTREAM_URLS: Record<string, string> = {
  searcher_rs: process.env["SEARCHER_URL"] ?? "http://searcher-rs:9001",
  selector_api: process.env["SELECTOR_URL"] ?? "http://selector-api:3002",
  sim_ctl: process.env["SIM_URL"] ?? "http://sim-ctl:3003",
  recon: process.env["RECON_URL"] ?? "http://recon:3004",
  relays_client: process.env["RELAYS_URL"] ?? "http://relays-client:3005",
};

/**
 * Calcula la entropía del sistema basada en múltiples fuentes de datos.
 * La entropía representa la dispersión del estado topológico del ecosistema.
 * Valor 0 = estado puro (máxima convergencia), 1 = máxima dispersión.
 *
 * R8 FAIL-HONEST: Si no hay datos (oppsDetected + oppsScored = 0), retorna
 * entropy = null para indicar ausencia de datos, no un valor por defecto.
 */
async function calculateEntropy(redis: Redis, pool: Pool | null): Promise<{
  entropy: number | null;
  raw_entropy: number | null;
  service_health_factor: number | null;
  delta: number;
  window_seconds: number;
  has_data: boolean;
}> {
  const windowSeconds = 60;
  const now = Date.now();

  try {
    // Obtener métricas de Redis para cálculo de entropía
    const [oppsDetected, oppsScored, heartbeatKeys] = await Promise.all([
      redis.xlen("arbx:opps:detected").catch(() => 0),
      redis.xlen("arbx:scoring:scored").catch(() => 0),
      redis.keys("arbx:heartbeat:*:latest").catch(() => []),
    ]);

    // R8 FAIL-HONEST: Sin datos = sin métrica significativa
    const totalOpps = oppsDetected + oppsScored;
    if (totalOpps === 0) {
      return {
        entropy: null,
        raw_entropy: null,
        service_health_factor: null,
        delta: 0,
        window_seconds: windowSeconds,
        has_data: false,
      };
    }

    // Calcular entropía basada en la distribución de oportunidades
    // Fórmula: H = -Σ(p_i * log(p_i)) / log(N) — normalizada [0,1]
    const total = Math.max(1, totalOpps);
    const pDetected = oppsDetected / total;
    const pScored = oppsScored / total;

    let rawEntropy = 0;
    if (pDetected > 0) rawEntropy -= pDetected * Math.log2(pDetected);
    if (pScored > 0) rawEntropy -= pScored * Math.log2(pScored);

    // Normalizar a [0,1] — máxima entropía binaria = 1
    const maxEntropy = Math.log2(2);
    rawEntropy = rawEntropy / maxEntropy;

    // Factor de salud de servicios (0 = todos caídos, 1 = todos saludables)
    const serviceHealthFactor = heartbeatKeys.length > 0
      ? Math.min(1, heartbeatKeys.length / 5)
      : 0;

    // Entropía final: combinación de distribución y salud de servicios
    // NOTA: Pesos 0.6/0.4 son heurísticos de monitoreo, no tienen fundamento físico.
    // Para análisis riguroso, usar raw_entropy y service_health_factor por separado.
    const finalEntropy = Number((rawEntropy * 0.6 + (1 - serviceHealthFactor) * 0.4).toFixed(4));

    // Calcular delta respecto a ventana anterior (simulado)
    const prevEntropyRaw = await redis.get("arbx:metrics:entropy:prev").catch(() => null);
    const prevEntropy = prevEntropyRaw !== null ? Number(prevEntropyRaw) : null;
    const delta = prevEntropy !== null ? Number((finalEntropy - prevEntropy).toFixed(4)) : 0;

    // Guardar para próximo cálculo
    await redis.setex("arbx:metrics:entropy:prev", windowSeconds * 2, String(finalEntropy));

    return {
      entropy: finalEntropy,
      raw_entropy: rawEntropy,
      service_health_factor: serviceHealthFactor,
      delta,
      window_seconds: windowSeconds,
      has_data: true,
    };
  } catch (e) {
    // R8 FAIL-HONEST: Error = sin datos, no valor por defecto
    return {
      entropy: null,
      raw_entropy: null,
      service_health_factor: null,
      delta: 0,
      window_seconds: windowSeconds,
      has_data: false,
    };
  }
}

/**
 * Verifica el estado de un servicio upstream vía health endpoint.
 */
async function checkService(name: string, url: string): Promise<ServiceStatus> {
  const start = Date.now();
  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 2000);
    const response = await fetch(`${url}/health`, {
      signal: controller.signal,
    });
    clearTimeout(timeout);

    const latency = Date.now() - start;

    if (response.ok) {
      return {
        status: "running",
        latency_ms: latency,
        last_check: new Date().toISOString(),
      };
    }

    return {
      status: "degraded",
      latency_ms: latency,
      last_check: new Date().toISOString(),
      detail: `HTTP ${response.status}`,
    };
  } catch (e) {
    return {
      status: "stopped",
      last_check: new Date().toISOString(),
      detail: (e as Error).message,
    };
  }
}

/**
 * Verifica la conexión a PostgreSQL.
 */
async function checkPostgres(pool: Pool | null): Promise<ServiceStatus> {
  if (!pool) {
    return {
      status: "stopped",
      last_check: new Date().toISOString(),
      detail: "pool_not_initialized",
    };
  }

  const start = Date.now();
  try {
    await pool.query("SELECT 1");
    return {
      status: "running",
      latency_ms: Date.now() - start,
      last_check: new Date().toISOString(),
    };
  } catch (e) {
    return {
      status: "stopped",
      last_check: new Date().toISOString(),
      detail: (e as Error).message,
    };
  }
}

/**
 * Verifica la conexión a Redis.
 */
async function checkRedis(redis: Redis): Promise<ServiceStatus> {
  const start = Date.now();
  try {
    await redis.ping();
    return {
      status: "running",
      latency_ms: Date.now() - start,
      last_check: new Date().toISOString(),
    };
  } catch (e) {
    return {
      status: "stopped",
      last_check: new Date().toISOString(),
      detail: (e as Error).message,
    };
  }
}

/**
 * Calcula métricas de convergencia del sistema.
 */
async function calculateConvergence(redis: Redis): Promise<{
  rate: number;
  target: number;
  variance: number;
}> {
  try {
    // Obtener métricas de convergencia desde Redis
    const [resolved, attempted] = await Promise.all([
      redis.get("arbx:topology:loops:resolved").catch(() => "0"),
      redis.get("arbx:topology:loops:attempted").catch(() => "1"),
    ]);

    const resolvedNum = parseInt(resolved ?? "0", 10) || 0;
    const attemptedNum = parseInt(attempted ?? "1", 10) || 1;

    // Tasa de convergencia: loops resueltos / intentados
    const rate = Number((resolvedNum / attemptedNum).toFixed(4));

    // Varianza estimada (simulada desde distribución)
    const variance = Number((1 - rate).toFixed(4));

    return {
      rate,
      target: 0.95, // Target canónico OMEGA
      variance,
    };
  } catch {
    return {
      rate: 0,
      target: 0.95,
      variance: 1,
    };
  }
}

/**
 * Obtiene métricas topológicas del ecosistema.
 */
async function getTopologyMetrics(redis: Redis, pool: Pool | null): Promise<{
  manifolds_observed: number;
  loops_resolved: number;
  decoherence_rate: number;
}> {
  try {
    const [manifolds, loops, decoherence] = await Promise.all([
      redis.scard("arbx:topology:manifolds").catch(() => 0),
      redis.get("arbx:topology:loops:resolved").catch(() => "0"),
      redis.get("arbx:topology:decoherence:rate").catch(() => "0"),
    ]);

    return {
      manifolds_observed: manifolds,
      loops_resolved: parseInt(loops ?? "0", 10) || 0,
      decoherence_rate: Number(parseFloat(decoherence ?? "0").toFixed(4)),
    };
  } catch {
    return {
      manifolds_observed: 0,
      loops_resolved: 0,
      decoherence_rate: 0,
    };
  }
}

/**
 * Monta las rutas de health y telemetría en el router Express.
 */
export function mountHealthRouter(deps: HealthDeps): Router {
  const router = Router();

  /**
   * GET /
   * Retorna el estado holístico del sistema OMEGA incluyendo:
   * - Estado de servicios críticos (searcher_rs, postgres, redis)
   * - Entropía termodinámica del sistema
   * - Métricas de convergencia y topología
   */
  router.get("/", async (_req: Request, res: Response) => {
    const timestamp = Math.floor(Date.now() / 1000);

    try {
      // Verificar todos los servicios en paralelo (Shotgun Dispatch)
      const [
        searcherStatus,
        selectorStatus,
        simStatus,
        reconStatus,
        relaysStatus,
        pgStatus,
        redisStatus,
        entropyData,
        convergenceData,
        topologyData,
      ] = await Promise.all([
        checkService("searcher_rs", UPSTREAM_URLS.searcher_rs ?? ""),
        checkService("selector_api", UPSTREAM_URLS.selector_api ?? ""),
        checkService("sim_ctl", UPSTREAM_URLS.sim_ctl ?? ""),
        checkService("recon", UPSTREAM_URLS.recon ?? ""),
        checkService("relays_client", UPSTREAM_URLS.relays_client ?? ""),
        checkPostgres(deps.pool),
        checkRedis(deps.redis),
        calculateEntropy(deps.redis, deps.pool),
        calculateConvergence(deps.redis),
        getTopologyMetrics(deps.redis, deps.pool),
      ]);

      // Determinar estado global del sistema
      const serviceStatuses = [
        { name: "searcher_rs", ...searcherStatus },
        { name: "selector_api", ...selectorStatus },
        { name: "sim_ctl", ...simStatus },
        { name: "recon", ...reconStatus },
        { name: "relays_client", ...relaysStatus },
        { name: "postgres", ...pgStatus },
        { name: "redis", ...redisStatus },
      ];
      const runningCount = serviceStatuses.filter(s => s.status === "running").length;
      const stoppedCount = serviceStatuses.filter(s => s.status === "stopped").length;

      let systemStatus: HealthResponse["system_status"] = "healthy";
      if (stoppedCount > 0) systemStatus = "degraded";
      if (stoppedCount > 3 || pgStatus.status !== "running") systemStatus = "critical";

      // Math Guardian: verificar invariantes matemáticos
      let mathGuardian: HealthResponse["math_guardian"] = "passed";
      const entropyValue = entropyData.entropy;
      if (entropyValue !== null) {
        if (entropyValue > 0.9) mathGuardian = "warning";
        if (entropyValue > 0.95 || convergenceData.variance > 0.5) mathGuardian = "failed";
      } else {
        // Sin datos de entropía = warning (no failed, es estado transitorio)
        mathGuardian = "warning";
      }

      const response: HealthResponse = {
        system_status: systemStatus,
        math_guardian: mathGuardian,
        entropy: entropyData.entropy ?? 1.0, // 1.0 = máxima incertidumbre cuando no hay datos
        timestamp,
        chains: OBSERVED_CHAINS,
        services: {
          searcher_rs: searcherStatus,
          selector_api: selectorStatus,
          sim_ctl: simStatus,
          recon: reconStatus,
          relays_client: relaysStatus,
          postgres: pgStatus,
          redis: redisStatus,
        },
        convergence: convergenceData,
        topology: topologyData,
      };

      const statusCode = systemStatus === "critical" ? 503 : 200;
      res.status(statusCode).json(response);
    } catch (e) {
      // Fail-honest: retornar estado crítico ante error inesperado
      res.status(503).json({
        system_status: "critical",
        math_guardian: "failed",
        entropy: 1.0,
        timestamp,
        chains: OBSERVED_CHAINS,
        services: {
          searcher_rs: { status: "unknown", last_check: new Date().toISOString() },
          postgres: { status: "unknown", last_check: new Date().toISOString() },
          redis: { status: "unknown", last_check: new Date().toISOString() },
        },
        convergence: { rate: 0, target: 0.95, variance: 1 },
        topology: { manifolds_observed: 0, loops_resolved: 0, decoherence_rate: 0 },
        error: (e as Error).message,
      });
    }
  });

  /**
   * GET /metrics/entropy
   * Retorna la entropía actual del sistema con su delta temporal.
   * FAIL-HONEST: entropy = null cuando no hay datos suficientes.
   */
  router.get("/metrics/entropy", async (_req: Request, res: Response) => {
    try {
      const entropyData = await calculateEntropy(deps.redis, deps.pool);

      const response: EntropyResponse = {
        entropy: entropyData.entropy,
        delta: entropyData.delta,
        timestamp: Math.floor(Date.now() / 1000),
        window_seconds: entropyData.window_seconds,
        has_data: entropyData.has_data,
      };

      res.status(200).json(response);
    } catch (e) {
      res.status(503).json({
        error: "entropy_calculation_failed",
        detail: (e as Error).message,
      });
    }
  });

  /**
   * GET /metrics/convergence
   * Retorna métricas detalladas de convergencia del sistema.
   */
  router.get("/metrics/convergence", async (_req: Request, res: Response) => {
    try {
      const convergenceData = await calculateConvergence(deps.redis);
      res.status(200).json({
        ...convergenceData,
        timestamp: Math.floor(Date.now() / 1000),
      });
    } catch (e) {
      res.status(503).json({
        error: "convergence_calculation_failed",
        detail: (e as Error).message,
      });
    }
  });

  /**
   * GET /metrics/topology
   * Retorna métricas topológicas del ecosistema de variedades de liquidez.
   */
  router.get("/metrics/topology", async (_req: Request, res: Response) => {
    try {
      const topologyData = await getTopologyMetrics(deps.redis, deps.pool);
      res.status(200).json({
        ...topologyData,
        timestamp: Math.floor(Date.now() / 1000),
      });
    } catch (e) {
      res.status(503).json({
        error: "topology_calculation_failed",
        detail: (e as Error).message,
      });
    }
  });

  return router;
}

/**
 * Configura el namespace de WebSocket para streaming de métricas.
 * Emite actualizaciones periódicas de entropía y convergencia.
 */
export function setupMetricsWebSocket(io: IoServer, redis: Redis, pool: Pool | null): void {
  const metricsNs = io.of("/ws/metrics");

  metricsNs.on("connection", (socket) => {
    // Enviar métricas iniciales
    const sendInitialMetrics = async () => {
      try {
        const [entropyData, convergenceData, topologyData] = await Promise.all([
          calculateEntropy(redis, pool),
          calculateConvergence(redis),
          getTopologyMetrics(redis, pool),
        ]);

        socket.emit("metrics:initial", {
          entropy: entropyData.entropy,
          raw_entropy: entropyData.raw_entropy,
          service_health_factor: entropyData.service_health_factor,
          delta: entropyData.delta,
          has_data: entropyData.has_data,
          convergence: convergenceData,
          topology: topologyData,
          timestamp: Math.floor(Date.now() / 1000),
        });
      } catch (e) {
        socket.emit("metrics:error", {
          error: "initial_metrics_failed",
          detail: (e as Error).message,
        });
      }
    };

    void sendInitialMetrics();

    // Configurar intervalo de actualización (cada 5 segundos)
    const interval = setInterval(async () => {
      try {
        const entropyData = await calculateEntropy(redis, pool);
        socket.emit("metrics:entropy", {
          entropy: entropyData.entropy,
          raw_entropy: entropyData.raw_entropy,
          service_health_factor: entropyData.service_health_factor,
          delta: entropyData.delta,
          has_data: entropyData.has_data,
          timestamp: Math.floor(Date.now() / 1000),
        });
      } catch (e) {
        socket.emit("metrics:error", {
          error: "entropy_update_failed",
          detail: (e as Error).message,
        });
      }
    }, 5000);

    socket.on("disconnect", () => {
      clearInterval(interval);
    });
  });
}
