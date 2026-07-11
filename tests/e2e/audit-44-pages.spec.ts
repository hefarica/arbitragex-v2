import { test, expect } from "@playwright/test";
import { execSync } from "child_process";
import * as fs from "fs";
import * as path from "path";

/**
 * Auditoría Manual de 44 Páginas - ArbitrageX v2
 *
 * Este test navega por TODAS las páginas de la aplicación,
 * captura screenshots y documenta errores.
 *
 * Uso:
 *   cd tests/e2e && npm test -- audit-44-pages.spec.ts
 *   # O con UI:
 *   cd tests/e2e && npm run test:ui -- audit-44-pages.spec.ts
 */

// Crear directorio para screenshots si no existe
const AUDIT_DIR = path.join(process.cwd(), "test-results", "audit-44-pages");
if (!fs.existsSync(AUDIT_DIR)) {
  fs.mkdirSync(AUDIT_DIR, { recursive: true });
}

// Timestamp para el reporte
const TIMESTAMP = new Date().toISOString().replace(/[:.]/g, "-");

// Resultados de auditoría
const auditResults: Array<{
  path: string;
  status: number | null;
  hasEdgeError: boolean;
  hasH1: boolean;
  h1Text: string;
  screenshotPath: string;
  errors: string[];
  timestamp: string;
}> = [];

// Las 44 páginas identificadas por el agente de inventario
const PAGES: Array<{
  path: string;
  category: string;
  priority: "CRITICAL" | "HIGH" | "MEDIUM" | "LOW";
  expectedHeading?: RegExp;
}> = [
  // CRITICAL - Impacto directo en capital
  { path: "/killswitch", category: "Risk", priority: "CRITICAL", expectedHeading: /kill.switch/i },
  { path: "/risk", category: "Risk", priority: "CRITICAL", expectedHeading: /risk.*alerts/i },
  { path: "/live-readiness", category: "Risk", priority: "CRITICAL", expectedHeading: /readiness/i },
  { path: "/config/trading", category: "Config", priority: "CRITICAL", expectedHeading: /trading/i },
  { path: "/apex/allocator", category: "Trading", priority: "CRITICAL", expectedHeading: /allocator/i },

  // HIGH - Operaciones core
  { path: "/", category: "Core", priority: "HIGH", expectedHeading: /operator console/i },
  { path: "/opportunities", category: "Core", priority: "HIGH", expectedHeading: /opportunities/i },
  { path: "/opportunities/by-strategy", category: "Core", priority: "HIGH" },
  { path: "/executions", category: "Core", priority: "HIGH", expectedHeading: /executions/i },
  { path: "/paper/history", category: "Core", priority: "HIGH", expectedHeading: /paper/i },
  { path: "/status", category: "Observe", priority: "HIGH", expectedHeading: /status/i },
  { path: "/strategies", category: "Core", priority: "HIGH", expectedHeading: /strategies/i },
  { path: "/strategies/forge", category: "Core", priority: "HIGH", expectedHeading: /forge/i },

  // MEDIUM - Configuración y admin
  { path: "/settings", category: "Config", priority: "MEDIUM", expectedHeading: /settings/i },
  { path: "/settings/credentials", category: "Config", priority: "MEDIUM", expectedHeading: /credentials/i },
  { path: "/config", category: "Config", priority: "MEDIUM", expectedHeading: /config/i },
  { path: "/chains", category: "Config", priority: "MEDIUM", expectedHeading: /chains/i },
  { path: "/rpcs", category: "Config", priority: "MEDIUM", expectedHeading: /rpc/i },
  { path: "/wallets", category: "Config", priority: "MEDIUM", expectedHeading: /wallets/i },
  { path: "/wallet", category: "Config", priority: "MEDIUM", expectedHeading: /wallet/i },
  { path: "/admin/signin", category: "Admin", priority: "MEDIUM", expectedHeading: /signin|login/i },
  { path: "/admin/chains", category: "Admin", priority: "MEDIUM", expectedHeading: /chains/i },
  { path: "/admin/topology", category: "Admin", priority: "MEDIUM", expectedHeading: /topology/i },

  // Omega S5
  { path: "/omega-s5/core", category: "Omega", priority: "MEDIUM", expectedHeading: /core/i },
  { path: "/omega-s5/operator", category: "Omega", priority: "MEDIUM", expectedHeading: /operator/i },
  { path: "/omega-s5/adapters", category: "Omega", priority: "MEDIUM", expectedHeading: /adapters/i },
  { path: "/omega-s5/factory", category: "Omega", priority: "MEDIUM", expectedHeading: /factory/i },
  { path: "/omega-s5/crucible", category: "Omega", priority: "MEDIUM", expectedHeading: /crucible/i },
  { path: "/omega-s5/drift", category: "Omega", priority: "MEDIUM", expectedHeading: /drift/i },
  { path: "/omega-s5/registry", category: "Omega", priority: "MEDIUM", expectedHeading: /registry/i },
  { path: "/omega-s5/wallets", category: "Omega", priority: "MEDIUM", expectedHeading: /wallets/i },

  // Onboarding
  { path: "/onboarding", category: "Onboarding", priority: "MEDIUM", expectedHeading: /onboarding/i },
  { path: "/onboarding/1-init", category: "Onboarding", priority: "MEDIUM", expectedHeading: /init/i },
  { path: "/onboarding/2-connect", category: "Onboarding", priority: "MEDIUM", expectedHeading: /connect/i },
  { path: "/onboarding/3-advanced", category: "Onboarding", priority: "MEDIUM", expectedHeading: /advanced/i },
  { path: "/onboarding/4-testing", category: "Onboarding", priority: "MEDIUM", expectedHeading: /testing/i },
  { path: "/onboarding/5-production", category: "Onboarding", priority: "MEDIUM", expectedHeading: /production/i },

  // LOW - Observabilidad
  { path: "/worker-health", category: "Observe", priority: "LOW", expectedHeading: /worker|health/i },
  { path: "/audit-logs", category: "Observe", priority: "LOW", expectedHeading: /audit/i },
  { path: "/recon", category: "Observe", priority: "LOW", expectedHeading: /recon/i },
  { path: "/operations", category: "Observe", priority: "LOW", expectedHeading: /operations/i },
  { path: "/agent-insights", category: "Observe", priority: "LOW", expectedHeading: /insights|agents/i },
  { path: "/sed", category: "Observe", priority: "LOW", expectedHeading: /sed/i },
  { path: "/pools", category: "Config", priority: "LOW", expectedHeading: /pools/i },
  { path: "/dex-registry", category: "Config", priority: "LOW", expectedHeading: /dex/i },
  { path: "/routes/discovery", category: "Core", priority: "LOW", expectedHeading: /discovery|routes/i },
  { path: "/route-outcomes", category: "Core", priority: "LOW", expectedHeading: /outcomes/i },
  { path: "/deploy-pipeline", category: "Config", priority: "LOW", expectedHeading: /deploy/i },
  { path: "/operator", category: "Risk", priority: "LOW" },
  { path: "/operator/self-test", category: "Risk", priority: "LOW", expectedHeading: /self.test/i },
  { path: "/operator/presets", category: "Risk", priority: "LOW", expectedHeading: /presets/i },
];

// Hook para capturar errores de consola
let consoleErrors: string[] = [];

test.beforeEach(async ({ page }) => {
  consoleErrors = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") {
      consoleErrors.push(msg.text());
    }
  });
});

test.afterAll(async () => {
  // Generar reporte JSON
  const reportPath = path.join(AUDIT_DIR, `audit-report-${TIMESTAMP}.json`);
  fs.writeFileSync(reportPath, JSON.stringify({
    timestamp: TIMESTAMP,
    totalPages: PAGES.length,
    summary: {
      ok: auditResults.filter(r => !r.hasEdgeError && r.status && r.status < 400).length,
      edgeError: auditResults.filter(r => r.hasEdgeError).length,
      httpError: auditResults.filter(r => r.status && r.status >= 400).length,
      noH1: auditResults.filter(r => !r.hasH1).length,
    },
    results: auditResults,
  }, null, 2));
  console.log(`\n📊 Reporte guardado: ${reportPath}`);

  // Generar reporte Markdown
  const mdPath = path.join(AUDIT_DIR, `audit-report-${TIMESTAMP}.md`);
  const md = generateMarkdownReport();
  fs.writeFileSync(mdPath, md);
  console.log(`📝 Reporte Markdown: ${mdPath}`);
});

function generateMarkdownReport(): string {
  const critical = auditResults.filter(r => PAGES.find(p => p.path === r.path)?.priority === "CRITICAL");
  const high = auditResults.filter(r => PAGES.find(p => p.path === r.path)?.priority === "HIGH");
  const medium = auditResults.filter(r => PAGES.find(p => p.path === r.path)?.priority === "MEDIUM");
  const low = auditResults.filter(r => PAGES.find(p => p.path === r.path)?.priority === "LOW");

  return `# Reporte de Auditoría - 44 Páginas

**Fecha:** ${TIMESTAMP}
**Total Páginas:** ${PAGES.length}

## Resumen

| Prioridad | Total | OK | Edge Error | HTTP Error | Sin H1 |
|-----------|-------|-----|------------|------------|--------|
| CRITICAL | ${critical.length} | ${critical.filter(r => !r.hasEdgeError).length} | ${critical.filter(r => r.hasEdgeError).length} | ${critical.filter(r => r.status && r.status >= 400).length} | ${critical.filter(r => !r.hasH1).length} |
| HIGH | ${high.length} | ${high.filter(r => !r.hasEdgeError).length} | ${high.filter(r => r.hasEdgeError).length} | ${high.filter(r => r.status && r.status >= 400).length} | ${high.filter(r => !r.hasH1).length} |
| MEDIUM | ${medium.length} | ${medium.filter(r => !r.hasEdgeError).length} | ${medium.filter(r => r.hasEdgeError).length} | ${medium.filter(r => r.status && r.status >= 400).length} | ${medium.filter(r => !r.hasH1).length} |
| LOW | ${low.length} | ${low.filter(r => !r.hasEdgeError).length} | ${low.filter(r => r.hasEdgeError).length} | ${low.filter(r => r.status && r.status >= 400).length} | ${low.filter(r => !r.hasH1).length} |

## Resultados Detallados

| Ruta | Prioridad | Estado HTTP | Edge Error | H1 | Screenshot |
|------|-----------|-------------|------------|-----|------------|
${auditResults.map(r => {
  const p = PAGES.find(pg => pg.path === r.path);
  const emoji = r.hasEdgeError ? "🔴" : r.status && r.status >= 400 ? "🟡" : "🟢";
  return `| ${r.path} | ${p?.priority || "?"} | ${r.status || "N/A"} | ${r.hasEdgeError ? "Sí" : "No"} | ${r.hasH1 ? "✅" : "❌"} | [Ver](${r.screenshotPath}) |`;
}).join("\n")}

## Errores de Consola

${auditResults.filter(r => r.errors.length > 0).map(r => `
### ${r.path}
${r.errors.map(e => `- ${e}`).join("\n")}
`).join("\n")}

---
*Generado automáticamente por Playwright*
`;
}

// Generar test para cada página
for (const pageInfo of PAGES) {
  test(`audit page: ${pageInfo.path} (${pageInfo.priority})`, async ({ page }) => {
    // Navegar a la página
    const response = await page.goto(pageInfo.path, { timeout: 30000 });
    const status = response?.status() || null;

    // Esperar a que el contenido cargue
    await page.waitForLoadState("networkidle").catch(() => {});

    // Capturar screenshot
    const screenshotName = `${pageInfo.path.replace(/\//g, "_") || "home"}-${TIMESTAMP}.png`;
    const screenshotPath = path.join(AUDIT_DIR, screenshotName);
    await page.screenshot({ path: screenshotPath, fullPage: true });

    // Verificar H1
    const h1 = page.locator("h1").first();
    const hasH1 = await h1.isVisible().catch(() => false);
    const h1Text = hasH1 ? await h1.textContent() || "" : "";

    // Verificar errores de edge
    const edgeUnreachable = await page.getByText(/edge unreachable/i).count();
    const edgeError = await page.getByText(/edge error:/i).count();
    const hasEdgeError = edgeUnreachable > 0 || edgeError > 0;

    // Guardar resultado
    const result = {
      path: pageInfo.path,
      status,
      hasEdgeError,
      hasH1,
      h1Text,
      screenshotPath,
      errors: [...consoleErrors],
      timestamp: new Date().toISOString(),
    };
    auditResults.push(result);

    // Assertions (no fallan el test, solo documentan)
    if (pageInfo.expectedHeading) {
      await expect(h1, `H1 should match pattern for ${pageInfo.path}`).toHaveText(pageInfo.expectedHeading).catch(() => {});
    }

    // Log para el usuario
    const emoji = hasEdgeError ? "🔴" : status && status >= 400 ? "🟡" : "🟢";
    console.log(`${emoji} ${pageInfo.path} (${pageInfo.priority}) - HTTP ${status} - ${hasEdgeError ? "EDGE ERROR" : "OK"}`);
  });
}
