import pino, { Logger, LoggerOptions } from "pino";
import pinoHttp from "pino-http";
import { randomUUID } from "node:crypto";

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error" | "fatal";

export interface CreateLoggerOpts {
  service: string;
  level?: LogLevel;
  extra?: Record<string, unknown>;
}

export function createLogger(opts: CreateLoggerOpts): Logger {
  const level = opts.level ?? (process.env["LOG_LEVEL"] as LogLevel) ?? "info";
  const base: LoggerOptions = {
    level,
    base: { service: opts.service, ...opts.extra },
    timestamp: pino.stdTimeFunctions.isoTime,
    formatters: {
      level: (label) => ({ level: label }),
    },
    redact: {
      paths: [
        "req.headers.authorization",
        "req.headers['x-arbx-admin-token']",
        "req.headers['x-arbx-edge-token']",
        "req.headers.cookie",
        "password",
        "secret",
        "privateKey",
        "private_key",
      ],
      censor: "[redacted]",
    },
  };
  return pino(base);
}

/** Express/Fastify-compatible HTTP logger middleware. Requires/propagates `x-arbx-trace-id`. */
export function createHttpLogger(service: string, level: LogLevel = "info") {
  const logger = createLogger({ service, level });
  return pinoHttp({
    logger,
    genReqId: (req: { headers: Record<string, string | string[] | undefined> }, res: { setHeader: (n: string, v: string) => void }) => {
      const existing = req.headers["x-arbx-trace-id"];
      const id = typeof existing === "string" && existing.length > 0 ? existing : randomUUID();
      res.setHeader("x-arbx-trace-id", id);
      return id;
    },
    customLogLevel: (_req: unknown, res: { statusCode: number }, err: Error | undefined) => {
      if (err || res.statusCode >= 500) return "error";
      if (res.statusCode >= 400) return "warn";
      return level;
    },
    serializers: {
      req: (req: { method?: string; url?: string; id?: string }) => ({
        method: req.method,
        url: req.url,
        traceId: req.id,
      }),
      res: (res: { statusCode: number }) => ({ statusCode: res.statusCode }),
    },
  });
}
