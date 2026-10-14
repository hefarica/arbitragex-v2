/**
 * EDGE-ROUTE-PARITY-01 (2026-09-29) — gate anti-regresión del hueco R10 que
 * QUANT-PAGE-01 encontró.
 *
 * El edge declara rutas UNA POR UNA: no hay `app.all("/api/*")`. Por lo tanto una
 * ruta nueva del api-server es invisible para el navegador (NEXT_PUBLIC_EDGE_URL)
 * y para el SSR (INTERNAL_EDGE_URL) hasta que alguien escribe su fila aquí. El
 * síntoma no es un error ruidoso: es un 404 con la misma pinta que "no hay datos".
 *
 * Este archivo protege las dos mitades del contrato:
 *   1. FUNCIONAL — pidiendo la ruta a través del edge con upstream simulado: si la
 *      fila desaparece, el edge responde 404 y el test cae. También verifica que el
 *      querystring del cliente (window_minutes/limit/top) llegue intacto al
 *      api-server, que es lo que hace viva a la ventana cuantitativa.
 *   2. ESTRUCTURAL — el path que el frontend usa de verdad (extraído del fuente de
 *      `frontend/lib/quant-layers.ts`, no copiado a mano) debe estar declarado en
 *      los DOS edges que existen (worker de producción y shim dev-local). Así un
 *      renombre en el frontend o en el edge rompe el test en vez de romper la
 *      pantalla en producción.
 */
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import app from "./index.js";

type Env = Parameters<typeof app.request>[2];

function makeKV() {
  const store = new Map<string, string>();
  return {
    store,
    async get(key: string): Promise<string | null> {
      return store.get(key) ?? null;
    },
    async put(key: string, value: string): Promise<void> {
      store.set(key, value);
    },
    async delete(key: string): Promise<void> {
      store.delete(key);
    },
  };
}

function makeEnv(): Env {
  return {
    ARBX_ENV: "test",
    API_SERVER_URL: "http://upstream.invalid",
    ALLOWED_ORIGINS: "",
    ARBX_EDGE_TOKEN: "edge-secret",
    JWT_SECRET: "jwt-secret",
    ARBX_CACHE: makeKV(),
    RATE_LIMIT: makeKV(),
  } as unknown as Env;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("EDGE-ROUTE-PARITY-01: /api/quant/layers es alcanzable por el borde", () => {
  it("no es 404: el edge proxya al api-server y conserva window_minutes/limit/top", async () => {
    const upstream = vi.fn(
      async (_input: unknown) =>
        new Response(JSON.stringify({ ok: true, window_minutes: 15, layers: {} }), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
    );
    vi.stubGlobal("fetch", upstream);

    const res = await app.request(
      new Request("http://edge.invalid/api/quant/layers?window_minutes=15&limit=500&top=10"),
      undefined,
      makeEnv(),
    );

    expect(res.status).toBe(200);
    // Pass-through deliberado: SIN KV cache (la capa es una ventana móvil).
    expect(res.headers.get("x-arbx-cache")).toBe("PASS");
    expect(upstream).toHaveBeenCalledTimes(1);
    expect(String(upstream.mock.calls[0]?.[0])).toBe(
      "http://upstream.invalid/api/quant/layers?window_minutes=15&limit=500&top=10",
    );
    expect(await res.json()).toMatchObject({ ok: true, window_minutes: 15 });
  });

  it("una ruta NO declarada sigue siendo 404 (el gate distingue declarado de no declarado)", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
    const res = await app.request(new Request("http://edge.invalid/api/quant/inexistente"), undefined, makeEnv());
    expect(res.status).toBe(404);
  });
});

describe("EDGE-ROUTE-PARITY-01: el path del frontend está declarado en ambos edges", () => {
  const edgeSrc = readFileSync(new URL("./index.ts", import.meta.url), "utf8");
  const devLocalSrc = readFileSync(new URL("../../dev-local/src/index.ts", import.meta.url), "utf8");
  const frontendLibSrc = readFileSync(new URL("../../../frontend/lib/quant-layers.ts", import.meta.url), "utf8");

  it("extrae del frontend el path real (no una copia a mano)", () => {
    const m = /["'`](\/api\/quant\/layers)/.exec(frontendLibSrc);
    expect(m?.[1]).toBe("/api/quant/layers");
  });

  it("worker de producción: app.get('/api/quant/layers') declarado", () => {
    expect(edgeSrc).toContain('app.get("/api/quant/layers"');
  });

  it("shim dev-local: misma fila (paridad entre los dos edges)", () => {
    expect(devLocalSrc).toContain('app.get("/api/quant/layers"');
  });

  it("el path declarado es exactamente el que el frontend pide", () => {
    const m = /["'`](\/api\/quant\/layers)/.exec(frontendLibSrc);
    const path = m?.[1] ?? "";
    expect(edgeSrc).toContain(`app.get("${path}"`);
    expect(devLocalSrc).toContain(`app.get("${path}"`);
  });
});
