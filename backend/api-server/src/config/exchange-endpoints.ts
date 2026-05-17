/**
 * Exchange endpoint configuration — sourced exclusively from environment variables.
 *
 * ## Doctrine: No-Hardcode (§B — URL operativa)
 *
 * Exchange base URLs are operational endpoints subject to change (geo-restrictions,
 * domain migrations, sandbox vs. production toggle) and MUST NOT be hardcoded
 * as silent fallbacks. The system must fail-fast on missing env rather than
 * silently connect to a potentially unintended production CEX / oracle endpoint.
 *
 * ## Environment variables
 *
 * | Variable                      | Example value                                  |
 * |-------------------------------|------------------------------------------------|
 * | `ENDPOINT_COINGECKO_FREE`     | https://api.coingecko.com                      |
 * | `ENDPOINT_COINGECKO_PRO`      | https://pro-api.coingecko.com                  |
 * | `ENDPOINT_ALCHEMY_PRICES`     | https://api.g.alchemy.com                      |
 * | `ENDPOINT_BLOXROUTE`          | https://api.bloxroute.com                      |
 * | `ENDPOINT_BINANCE`            | https://api.binance.com                        |
 * | `ENDPOINT_OKX`                | https://www.okx.com                            |
 * | `ENDPOINT_BYBIT`              | https://api.bybit.com                          |
 * | `ENDPOINT_DEXSCREENER`        | https://api.dexscreener.com                    |
 *
 * All variables must be present; absence or empty value causes a startup error.
 */

export interface ExchangeEndpoints {
  /** Coingecko free-tier base URL — used in credential validation ping. */
  readonly coingeckoFree: string;
  /** Coingecko Pro base URL — used with x-cg-pro-api-key header. */
  readonly coingeckoPro: string;
  /** Alchemy Prices API base URL. */
  readonly alchemyPrices: string;
  /** BloxRoute API base URL. */
  readonly bloxroute: string;
  /** Binance REST API base URL. */
  readonly binance: string;
  /** OKX REST API base URL. */
  readonly okx: string;
  /** Bybit REST API base URL. */
  readonly bybit: string;
  /** DexScreener API base URL. */
  readonly dexscreener: string;
}

/**
 * Load exchange endpoints from environment variables.
 *
 * Throws a descriptive error if any required variable is absent or empty.
 * There is NO silent fallback to a hardcoded value.
 * Doctrine: "POR EL HAZ DE LUZ SOLO PASA QUIEN ES VISTO".
 */
export function loadExchangeEndpoints(): ExchangeEndpoints {
  return {
    coingeckoFree: requireEnvUrl("ENDPOINT_COINGECKO_FREE"),
    coingeckoPro: requireEnvUrl("ENDPOINT_COINGECKO_PRO"),
    alchemyPrices: requireEnvUrl("ENDPOINT_ALCHEMY_PRICES"),
    bloxroute: requireEnvUrl("ENDPOINT_BLOXROUTE"),
    binance: requireEnvUrl("ENDPOINT_BINANCE"),
    okx: requireEnvUrl("ENDPOINT_OKX"),
    bybit: requireEnvUrl("ENDPOINT_BYBIT"),
    dexscreener: requireEnvUrl("ENDPOINT_DEXSCREENER"),
  };
}

/**
 * Load a URL from a named env variable, throwing on absent or empty values.
 *
 * @throws Error if the variable is not set or is an empty/whitespace string.
 */
function requireEnvUrl(varName: string): string {
  const raw = process.env[varName];
  if (!raw || raw.trim() === "") {
    throw new Error(
      `exchange endpoint config: env var \`${varName}\` is not set or empty. ` +
        `Add it to your .env file or deployment secrets. ` +
        `See docs/governance/NO-HARDCODE-DOCTRINE.md §B.`
    );
  }
  // Strip trailing slash for consistent URL construction.
  return raw.trim().replace(/\/+$/, "");
}

/**
 * Singleton instance. Eagerly validated at module load so startup fails
 * immediately if any env var is missing (fail-fast doctrine).
 *
 * Import this in production code:
 *   `import { endpoints } from "../config/exchange-endpoints"`
 *
 * In tests, construct a mock endpoints object directly rather than importing
 * this singleton (avoids requiring env vars in the test runner).
 */
let _endpoints: ExchangeEndpoints | null = null;

export function getExchangeEndpoints(): ExchangeEndpoints {
  if (!_endpoints) {
    _endpoints = loadExchangeEndpoints();
  }
  return _endpoints;
}

/**
 * Inject a pre-built endpoints object — **use only in tests**.
 *
 * Call this before any code path that calls `getExchangeEndpoints()`.
 *
 * @example
 * ```ts
 * import { injectEndpointsForTest } from "../config/exchange-endpoints";
 * injectEndpointsForTest({ binance: "http://localhost:9999", ... });
 * ```
 */
export function injectEndpointsForTest(endpoints: ExchangeEndpoints): void {
  _endpoints = endpoints;
}
