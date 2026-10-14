// SHIM SOLO PARA EL REPRO. No altera el parser: re-exporta los modulos REALES.
// `OpportunitySchema` sale de shared-ts/src/contracts/index.ts (el archivo real)
// y `CircuitBreakerOpenError` de shared-ts/src/circuit_breaker/index.ts (el real).
// El shim existe unicamente porque `shared-ts/src/index.ts` arrastra config/
// logging/metrics/middleware, que necesitan npm deps (smol-toml, ajv, pino,
// prom-client, express) ausentes en este arbol de node_modules.
export * from "../../shared-ts/src/contracts/index.js";
export * from "../../shared-ts/src/circuit_breaker/index.js";
