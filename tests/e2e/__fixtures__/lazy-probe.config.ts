/**
 * SONDA DE CONFIG (t29) — sólo para MEDIR la semántica de Playwright 1.61.
 * ¿Playwright evalúa `use.baseURL` de TODOS los proyectos al cargar el config
 * (eager), o sólo del proyecto seleccionado (lazy)?
 *
 * Resultado determina si es posible un requisito de presencia condicionado por
 * proyecto dentro de UN solo config, o si hace falta separar en dos configs.
 *
 * Este archivo NO es un config de producción; vive en __fixtures__ y no está
 * referenciado por ningún script.
 */
import { defineConfig, devices } from "@playwright/test";
import fs from "node:fs";

const MARK = process.env["ARBX_PROBE_MARK"] ?? "probe-mark.txt";
const mark = (tag: string) => {
  try {
    fs.appendFileSync(MARK, `${tag}\n`);
  } catch {
    /* noop */
  }
};

mark("config-load:start");

export default defineConfig({
  testDir: "..",
  projects: [
    {
      name: "probe-frontend",
      testMatch: "smoke.spec.ts",
      use: {
        ...devices["Desktop Chrome"],
        get baseURL(): string {
          mark("frontend-project:baseURL-read");
          return "http://probe-frontend.invalid/";
        },
      },
    },
    {
      name: "probe-hotpath",
      testMatch: "hot-path-pipeline.spec.ts",
      use: {
        ...devices["Desktop Chrome"],
        get baseURL(): string {
          mark("hotpath-project:baseURL-read");
          return "http://probe-edge.invalid/";
        },
      },
    },
  ],
});

mark("config-load:end");
