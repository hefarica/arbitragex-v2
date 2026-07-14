# PLAYWRIGHT VPS DAPP SCAFFOLD SUPREME OMEGA - GUIA DE EJECUCION CON PLAYWRIGHT

**Version:** 1.0.0
**Modulo:** playwright-execution-guide
**Estado:** REQUIRED
**Dependencies:** README-1.md, README-2.md
**Pre-requisitos:** Node.js 20+, Docker, Git

---

## 1. ARQUITECTURA DE TESTING CON PLAYWRIGHT

### Componentes del Sistema de Testing

```
┌─────────────────────────────────────────────────────────────┐
│                    PLAYWRIGHT TEST RUNNER                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Browser   │  │   Tracing   │  │   Screenshot/HAR   │  │
│  │  Chromium   │  │   Engine    │  │      Capture        │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
└─────────┼────────────────┼────────────────────┼─────────────┘
          │                │                    │
          ▼                ▼                    ▼
┌─────────────────────────────────────────────────────────────┐
│                     DAPP UNDER TEST                          │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌────────────────┐  │
│  │Frontend │  │  Edge   │  │  API    │  │   Contracts    │  │
│  │  5173   │──│  8787   │──│  3000   │──│   Testnet      │  │
│  └─────────┘  └─────────┘  └────┬────┘  └────────────────┘  │
│                                 │                           │
│                    ┌────────────┴────────────┐              │
│                    ▼                         ▼              │
│              ┌──────────┐              ┌──────────┐         │
│              │  Redis   │              │Postgres  │         │
│              │  6379    │              │  5432    │         │
│              └──────────┘              └──────────┘         │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. CONFIGURACION INICIAL

### Paso 1: Instalacion de Dependencias

```bash
# Verificar Node.js version
node --version
# Expected: v20.11.0 or higher

# Instalar dependencias del workspace
npm install

# Instalar Playwright y browsers
cd tests/e2e
npm install
npx playwright install chromium
```

### Paso 2: Configuracion de Entorno

```bash
# Copiar archivo de ejemplo
cp .env.example .env

# Editar .env con valores locales
# NOTA: NUNCA commitear .env real
```

### Paso 3: Iniciar Infraestructura

```bash
# Iniciar Docker Compose stack
docker compose --env-file .env -f docker/compose.dev.yml up -d

# Verificar servicios saludables
docker compose ps

# Esperar a que postgres este listo
for i in {1..30}; do
  docker exec arbitragex-v2-postgres-1 pg_isready -U postgres && break
  sleep 1
done

# Aplicar migraciones
bash ./automation/scripts/migrate.sh
```

---

## 3. EJECUCION DE TESTS

### Comandos Basicos

```bash
# Todos los tests
cd tests/e2e
npm test

# Tests de humo (smoke)
npm run test:smoke

# Pipeline de alto rendimiento
npm run test:hotpath

# Tests en modo headed (visible)
npm test -- --headed

# Tests en modo UI
npm test -- --ui

# Tests especificos por nombre
npm test -- --grep "home loads"
```

### Ejecucion por Categoria

```bash
# Tests de caracterizacion
npm test -- characterization/

# Tests de interaccion
npm test -- interactions/

# Tests Web3
npm test -- web3-safe/

# Tests de pagos
npm test -- payments-safe/
```

---

## 4. ESCRITURA DE TESTS

### Test Basico

```typescript
import { test, expect } from "@playwright/test";

test("homepage has correct title", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/ArbitrageX/);
  await expect(page.locator("h1")).toBeVisible();
});
```

### Test con WebSocket

```typescript
import { test, expect } from "@playwright/test";
import { io } from "socket.io-client";

test("WebSocket connects and receives data", async () => {
  const WS_URL = process.env["ARBX_WS_URL"] ?? "http://localhost:3000";
  const socket = io(WS_URL);
  
  await new Promise<void>((resolve) => {
    socket.on("connect", resolve);
  });
  
  socket.emit("subscribe:opportunities");
  
  const data = await new Promise((resolve) => {
    socket.on("opportunity:detected", resolve);
  });
  
  expect(data).toHaveProperty("id");
  socket.disconnect();
});
```

---

## 5. CI/CD INTEGRACION

### GitHub Actions Workflow

```yaml
name: E2E Tests

on:
  pull_request:
    branches: [main]

jobs:
  playwright:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: actions/setup-node@v6
        with:
          node-version: "20"
          
      - name: Install Playwright
        working-directory: tests/e2e
        run: |
          npm install
          npx playwright install --with-deps chromium
          
      - name: Start services
        run: docker compose up -d
          
      - name: Run E2E tests
        working-directory: tests/e2e
        run: npm test
        
      - name: Upload report
        if: always()
        uses: actions/upload-artifact@v7
        with:
          name: playwright-report
          path: tests/e2e/playwright-report
```

---

**Status:** GUIA DE EJECUCION COMPLETADA
**Next:** README-4.md (Reporting de 40 Secciones)
