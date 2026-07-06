/**
 * Ω-S5++ C9.∞ APEX · Typed API client
 * -------------------------------------------------------------------------
 * Cliente HTTP estricto. SIEMPRE parsea con Zod (request + response).
 *
 * REGLAS:
 *  - Prohibido `any`, `as any`, `unknown as`.
 *  - Toda respuesta retorna `ApiResult<T>` (unión discriminada).
 *  - GET no usa idempotency_key. POST/PUT/PATCH/DELETE sí.
 *  - Header `X-Idempotency-Key` obligatorio en mutaciones.
 *  - Header `X-Operator-Token` obligatorio si requiere auth.
 *  - Header `X-Operator-Role` informativo (validado en backend).
 *  - Edge proxy: nunca llamar internal services — solo `/api/*` o `/edge/*`.
 *  - Toda firma sovereign va en body como `sovereign_signature` con payload_hash.
 */
import { z } from 'zod';
import {
  BlockedResponseSchema,
  UnavailableResponseSchema,
  ErrorResponseSchema,
  type BlockedResponse,
  type UnavailableResponse,
  type ErrorResponse,
  type IdempotencyKey,
} from '../schemas';

// ── ApiResult: unión discriminada universal ─────────────────────────────
export type ApiResult<T> =
  | { kind: 'ok'; data: T }
  | { kind: 'blocked'; data: BlockedResponse }
  | { kind: 'unavailable'; data: UnavailableResponse }
  | { kind: 'error'; data: ErrorResponse }
  | { kind: 'network_error'; message: string }
  | { kind: 'parse_error'; message: string; issues: z.ZodIssue[] };

// ── Mutation method types ───────────────────────────────────────────────
export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

export interface ApiCallOptions<TReq> {
  readonly path: string;
  readonly method: HttpMethod;
  readonly body?: TReq;
  readonly idempotencyKey?: IdempotencyKey;
  readonly operatorToken?: string;
  readonly signal?: AbortSignal;
  readonly timeoutMs?: number;
}

export interface ApiClientConfig {
  readonly edgeBaseUrl: string;
  readonly defaultTimeoutMs: number;
}

const DEFAULT_TIMEOUT_MS = 15_000;

// ── Validation: mutation method requires idempotency key ────────────────
const MUTATION_METHODS: ReadonlySet<HttpMethod> = new Set(['POST', 'PUT', 'PATCH', 'DELETE']);

function assertIdempotencyForMutation(method: HttpMethod, key?: IdempotencyKey): void {
  if (MUTATION_METHODS.has(method) && !key) {
    throw new Error(
      `INV-4 Idempotency violation: ${method} ${' '}requires idempotency_key. Refusing to fire.`,
    );
  }
}

// ── Validate response shape against universal schemas first ─────────────
function tryParseUniversalFailure(
  json: unknown,
):
  | { kind: 'blocked'; data: BlockedResponse }
  | { kind: 'unavailable'; data: UnavailableResponse }
  | { kind: 'error'; data: ErrorResponse }
  | null {
  const blocked = BlockedResponseSchema.safeParse(json);
  if (blocked.success) return { kind: 'blocked', data: blocked.data };

  const unavailable = UnavailableResponseSchema.safeParse(json);
  if (unavailable.success) return { kind: 'unavailable', data: unavailable.data };

  const error = ErrorResponseSchema.safeParse(json);
  if (error.success) return { kind: 'error', data: error.data };

  return null;
}

// ── Main typed fetcher ──────────────────────────────────────────────────
export async function apiCall<TReq, TRes>(
  config: ApiClientConfig,
  requestSchema: z.ZodSchema<TReq> | null,
  responseSchema: z.ZodSchema<TRes>,
  options: ApiCallOptions<TReq>,
): Promise<ApiResult<TRes>> {
  assertIdempotencyForMutation(options.method, options.idempotencyKey);

  // Validate outgoing body
  if (options.body !== undefined && requestSchema !== null) {
    const parsed = requestSchema.safeParse(options.body);
    if (!parsed.success) {
      return {
        kind: 'parse_error',
        message: 'Request body failed schema validation',
        issues: parsed.error.issues,
      };
    }
  }

  // Build headers (strict, no leaks)
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  };
  if (options.idempotencyKey) headers['X-Idempotency-Key'] = options.idempotencyKey;
  if (options.operatorToken) headers['X-Operator-Token'] = options.operatorToken;

  // Timeout via AbortController
  const controller = new AbortController();
  const timeoutMs = options.timeoutMs ?? config.defaultTimeoutMs;
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

  if (options.signal) {
    options.signal.addEventListener('abort', () => controller.abort(), { once: true });
  }

  const url = `${config.edgeBaseUrl}${options.path}`;

  let response: Response;
  try {
    response = await fetch(url, {
      method: options.method,
      headers,
      body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
      signal: controller.signal,
      cache: 'no-store',
      credentials: 'omit',
    });
  } catch (err) {
    clearTimeout(timeoutId);
    const message = err instanceof Error ? err.message : 'unknown network error';
    return { kind: 'network_error', message };
  }
  clearTimeout(timeoutId);

  let json: unknown;
  try {
    json = await response.json();
  } catch {
    return {
      kind: 'network_error',
      message: `HTTP ${response.status}: response is not valid JSON`,
    };
  }

  // First try universal failure shapes (BLOCKED / UNAVAILABLE / ERROR)
  const universalFailure = tryParseUniversalFailure(json);
  if (universalFailure) return universalFailure;

  // Then try the success schema
  const ok = responseSchema.safeParse(json);
  if (!ok.success) {
    return {
      kind: 'parse_error',
      message: `Response failed schema validation (HTTP ${response.status})`,
      issues: ok.error.issues,
    };
  }

  return { kind: 'ok', data: ok.data };
}

// ── Convenience helpers ─────────────────────────────────────────────────
export function buildApiClient(config: ApiClientConfig) {
  return {
    get: <TRes>(
      path: string,
      responseSchema: z.ZodSchema<TRes>,
      opts?: Omit<ApiCallOptions<never>, 'path' | 'method' | 'body'>,
    ): Promise<ApiResult<TRes>> =>
      apiCall<never, TRes>(config, null, responseSchema, { path, method: 'GET', ...opts }),

    post: <TReq, TRes>(
      path: string,
      requestSchema: z.ZodSchema<TReq>,
      responseSchema: z.ZodSchema<TRes>,
      body: TReq,
      idempotencyKey: IdempotencyKey,
      opts?: Omit<ApiCallOptions<TReq>, 'path' | 'method' | 'body' | 'idempotencyKey'>,
    ): Promise<ApiResult<TRes>> =>
      apiCall<TReq, TRes>(config, requestSchema, responseSchema, {
        path,
        method: 'POST',
        body,
        idempotencyKey,
        ...opts,
      }),

    put: <TReq, TRes>(
      path: string,
      requestSchema: z.ZodSchema<TReq>,
      responseSchema: z.ZodSchema<TRes>,
      body: TReq,
      idempotencyKey: IdempotencyKey,
      opts?: Omit<ApiCallOptions<TReq>, 'path' | 'method' | 'body' | 'idempotencyKey'>,
    ): Promise<ApiResult<TRes>> =>
      apiCall<TReq, TRes>(config, requestSchema, responseSchema, {
        path,
        method: 'PUT',
        body,
        idempotencyKey,
        ...opts,
      }),

    delete: <TRes>(
      path: string,
      responseSchema: z.ZodSchema<TRes>,
      idempotencyKey: IdempotencyKey,
      opts?: Omit<ApiCallOptions<never>, 'path' | 'method' | 'body' | 'idempotencyKey'>,
    ): Promise<ApiResult<TRes>> =>
      apiCall<never, TRes>(config, null, responseSchema, {
        path,
        method: 'DELETE',
        idempotencyKey,
        ...opts,
      }),
  } as const;
}

export type TypedApiClient = ReturnType<typeof buildApiClient>;
