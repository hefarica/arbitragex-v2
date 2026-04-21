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
/** Express/Fastify-compatible HTTP logger. Uses `any` in callbacks to avoid
 *  overload conflicts between genReqId and customLogLevel — pino-http's
 *  generic Options<Req,Res> doesn't compose cleanly under strict TS.
 */
export function createHttpLogger(service: string, level: LogLevel = "info") {
  const logger = createLogger({ service, level });
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const handler: any = pinoHttp({
    logger,
    genReqId: (req: any, res: any): string => {
      const existing = req.headers?.["x-arbx-trace-id"];
      const id = typeof existing === "string" && existing.length > 0 ? existing : randomUUID();
      if (typeof res.setHeader === "function") res.setHeader("x-arbx-trace-id", id);
      return id;
    },
    customLogLevel: (_req: any, res: any, err: any): LogLevel => {
      if (err || (res && res.statusCode >= 500)) return "error";
      if (res && res.statusCode >= 400) return "warn";
      return level;
    },
    serializers: {
      req: (req: any) => ({
        method: req.method,
        url: req.url,
        traceId: req.id,
      }),
      res: (res: any) => ({ statusCode: res.statusCode }),
    },
  });
  return handler;
}
