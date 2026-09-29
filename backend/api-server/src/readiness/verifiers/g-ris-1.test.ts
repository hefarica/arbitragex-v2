/**
 * G-RIS-1 — tests del verificador de riesgo (KS-KEY-01 + KS-TOML-01).
 *
 * Los dos defectos que estos tests bloquean, medidos el 2026-09-29:
 *
 *   KS-KEY-01 — la capa 2 leía `arbx:killswitch:enabled`, clave que NINGÚN camino
 *   de ejecución lee (el cliente canónico usa `arbx:killswitch`), y encima
 *   descartaba el valor: podía declarar verde sobre una clave sin escritor.
 *
 *   KS-TOML-01 — el flag de auto-trip se buscaba con regex sobre el texto CRUDO
 *   de app.toml, así que una línea comentada (`# auto_trip... = true`) daba verde.
 */
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Redis falso: la clase se instancia dentro del verificador, así que se mockea
// el módulo entero y se controla `get` por test.
const redisGet = vi.fn<[], Promise<string | null>>();
const redisConnect = vi.fn(async () => undefined);
const redisDisconnect = vi.fn(() => undefined);

vi.mock("ioredis", () => ({
  Redis: class {
    constructor(..._args: unknown[]) {}
    connect = redisConnect;
    disconnect = redisDisconnect;
    get = redisGet;
  },
}));

const { stripTomlComments, verifyGRIS1 } = await import("./g-ris-1.js");

const BASE_TOML = `
[risk]
max_position_size_usd = 500
max_revert_rate_pct = 5
auto_trip_on_high_revert_rate = true
`;

let repo: string;

async function writeCfg(body: string): Promise<void> {
  await mkdir(path.join(repo, "configs"), { recursive: true });
  await writeFile(path.join(repo, "configs", "app.toml"), body, "utf8");
}

beforeEach(async () => {
  repo = await mkdtemp(path.join(tmpdir(), "g-ris-1-"));
  redisGet.mockReset();
  redisConnect.mockClear();
  redisDisconnect.mockClear();
});

afterEach(async () => {
  await rm(repo, { recursive: true, force: true });
});

describe("stripTomlComments", () => {
  it("descarta comentarios de línea completa y al final de línea", () => {
    const out = stripTomlComments('a = 1 # activo?\n# b = true\nc = "x # no"');
    expect(out).toContain("a = 1 ");
    expect(out).not.toContain("activo?");
    expect(out).not.toContain("b = true");
    // El `#` dentro de comillas NO es comentario.
    expect(out).toContain('c = "x # no"');
  });
});

describe("G-RIS-1 · KS-TOML-01: una clave comentada no es una clave activa", () => {
  it("con el flag SÓLO en un comentario, el gate NO puede dar verde", async () => {
    await writeCfg(`
[risk]
max_position_size_usd = 500
max_revert_rate_pct = 5
# auto_trip_on_high_revert_rate = true
`);
    redisGet.mockResolvedValue(null); // clave ausente
    const item = await verifyGRIS1({ repo, redisUrl: "redis://fake:6379" });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("commented out");
  });

  it("con el flag activo, da verde y lo cita como evidencia", async () => {
    await writeCfg(BASE_TOML);
    redisGet.mockResolvedValue(null);
    const item = await verifyGRIS1({ repo, redisUrl: "redis://fake:6379" });
    expect(item.status).toBe("green");
    expect(item.reason).toContain("auto_trip_on_high_revert_rate=true");
  });
});

describe("G-RIS-1 · KS-KEY-01: la clave canónica se lee Y se usa", () => {
  it("lee `arbx:killswitch` (no la clave muerta) y reporta el estado ARMED", async () => {
    await writeCfg(BASE_TOML);
    redisGet.mockResolvedValue(
      JSON.stringify({
        enabled: true,
        reason: "emergency",
        triggered_by: "operator:hector",
        updated_at: "2026-09-29T05:00:00Z",
      }),
    );
    const item = await verifyGRIS1({ repo, redisUrl: "redis://fake:6379" });
    expect(redisGet).toHaveBeenCalledWith("arbx:killswitch");
    expect(item.reason).toContain("ARMED por operator:hector");
    expect(item.evidence).toMatchObject({ ref: "configs/app.toml + redis:arbx:killswitch" });
  });

  it("clave ausente: sigue siendo alcanzable (default de app.toml) y lo dice", async () => {
    await writeCfg(BASE_TOML);
    redisGet.mockResolvedValue(null);
    const item = await verifyGRIS1({ repo, redisUrl: "redis://fake:6379" });
    expect(item.status).toBe("green");
    expect(item.reason).toContain("clave ausente");
  });

  it("JSON que no parsea: amarillo (el cliente Rust fail-closed lo trataría como ARMED)", async () => {
    await writeCfg(BASE_TOML);
    redisGet.mockResolvedValue("1"); // el formato que el runbook muerto instruía
    const item = await verifyGRIS1({ repo, redisUrl: "redis://fake:6379" });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("forma de KillSwitchState");
  });

  it("Redis inalcanzable: amarillo honesto", async () => {
    await writeCfg(BASE_TOML);
    redisConnect.mockRejectedValueOnce(new Error("ECONNREFUSED 127.0.0.1:6379"));
    const item = await verifyGRIS1({ repo, redisUrl: "redis://fake:6379" });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("unreachable");
  });
});
