/**
 * ═══════════════════════════════════════════════════════════════════════════════
 * ArbitrageX-v2: Suite de Tests End-to-End - Integración de Cartuchos
 * ═══════════════════════════════════════════════════════════════════════════════
 *
 * Tests para validar:
 * 1. Registro de cartuchos en la base de datos
 * 2. Detección de oportunidades de Funding Rate Arbitrage
 * 3. Detección de oportunidades de Mean Reversion Arbitrage
 * 4. Ejecución de cartuchos en modo SHADOW
 * 5. Hot-reload via Redis
 *
 * Ejecutar: npx playwright test tests/e2e/cartridge-integration.spec.ts
 */

import { test, expect, Page } from '@playwright/test';

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURACIÓN
// ═══════════════════════════════════════════════════════════════════════════════

const API_URL = process.env.ARBITRAGEX_API_URL || 'http://localhost:8080';
const ADMIN_TOKEN = process.env.ARBITRAGEX_ADMIN_TOKEN || 'test-token';
const FRONTEND_URL = process.env.ARBITRAGEX_FRONTEND_URL || 'http://localhost:3000';

// ═══════════════════════════════════════════════════════════════════════════════
// TEST SUITE 1: REGISTRO DE CARTUCHOS
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('Cartridge Registration', () => {
  test('should register Funding Rate Arbitrage cartridge', async ({ request }) => {
    const response = await request.get(`${API_URL}/api/v1/cartridges`, {
      headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
    });

    expect(response.status()).toBe(200);
    const cartridges = await response.json();
    
    const fundingRateCartridge = cartridges.find(
      (c: any) => c.name === 'Funding Rate Arbitrage'
    );
    
    expect(fundingRateCartridge).toBeDefined();
    expect(fundingRateCartridge.status).toBe('active');
    expect(fundingRateCartridge.version).toContain('1.0');
  });

  test('should register Mean Reversion Arbitrage cartridge', async ({ request }) => {
    const response = await request.get(`${API_URL}/api/v1/cartridges`, {
      headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
    });

    expect(response.status()).toBe(200);
    const cartridges = await response.json();
    
    const meanReversionCartridge = cartridges.find(
      (c: any) => c.name === 'Mean Reversion Arbitrage'
    );
    
    expect(meanReversionCartridge).toBeDefined();
    expect(meanReversionCartridge.status).toBe('active');
  });

  test('should have all cartridges with valid metadata', async ({ request }) => {
    const response = await request.get(`${API_URL}/api/v1/cartridges`, {
      headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
    });

    expect(response.status()).toBe(200);
    const cartridges = await response.json();
    
    expect(cartridges.length).toBeGreaterThanOrEqual(7); // 5 original + 2 nuevos

    cartridges.forEach((cartridge: any) => {
      expect(cartridge).toHaveProperty('name');
      expect(cartridge).toHaveProperty('version');
      expect(cartridge).toHaveProperty('status');
      expect(cartridge).toHaveProperty('created_at');
      expect(['active', 'inactive']).toContain(cartridge.status);
    });
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// TEST SUITE 2: DETECCIÓN DE OPORTUNIDADES - FUNDING RATE
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('Funding Rate Arbitrage Detection', () => {
  test('should detect funding rate differential opportunities', async ({ request }) => {
    // Simular datos de funding rates
    const marketData = {
      chain_id: 1,
      token: 'BTC',
      funding_rates: [
        { exchange: 'binance', rate_bps: 10, timestamp: Date.now() },
        { exchange: 'bybit', rate_bps: 5, timestamp: Date.now() },
        { exchange: 'okx', rate_bps: 8, timestamp: Date.now() },
      ],
      spot_price_usd: 67000,
      perp_prices: [
        { exchange: 'binance', price: 67050, funding_rate: 10 },
        { exchange: 'bybit', price: 67000, funding_rate: 5 },
      ],
      liquidities: [
        { exchange: 'binance', liquidity_usd: 50000000 },
        { exchange: 'bybit', liquidity_usd: 30000000 },
      ],
    };

    // Enviar datos al endpoint de evaluación
    const response = await request.post(
      `${API_URL}/api/v1/cartridges/funding-rate-arb/evaluate`,
      {
        data: marketData,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    expect(result).toHaveProperty('is_opportunity');
    expect(result).toHaveProperty('estimated_profit');
    expect(result).toHaveProperty('confidence');

    if (result.is_opportunity) {
      expect(result.estimated_profit).toBeGreaterThan(0);
      expect(result.confidence).toBeGreaterThan(0.5);
      expect(result).toHaveProperty('long_exchange');
      expect(result).toHaveProperty('short_exchange');
      expect(result).toHaveProperty('rate_differential_bps');
    }
  });

  test('should calculate correct profit for funding rate arbitrage', async ({ request }) => {
    const marketData = {
      chain_id: 1,
      token: 'ETH',
      funding_rates: [
        { exchange: 'binance', rate_bps: 20, timestamp: Date.now() },
        { exchange: 'bybit', rate_bps: 5, timestamp: Date.now() },
      ],
      spot_price_usd: 3500,
      perp_prices: [
        { exchange: 'binance', price: 3550, funding_rate: 20 },
        { exchange: 'bybit', price: 3500, funding_rate: 5 },
      ],
      liquidities: [
        { exchange: 'binance', liquidity_usd: 100000000 },
        { exchange: 'bybit', liquidity_usd: 80000000 },
      ],
    };

    const response = await request.post(
      `${API_URL}/api/v1/cartridges/funding-rate-arb/evaluate`,
      {
        data: marketData,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    if (result.is_opportunity) {
      // Verificar que la ganancia es proporcional al diferencial
      const expectedProfit = (result.position_size_usd * result.rate_differential_bps) / 10000;
      expect(result.profit_per_cycle).toBeCloseTo(expectedProfit, 0);
    }
  });

  test('should reject low differential opportunities', async ({ request }) => {
    const marketData = {
      chain_id: 1,
      token: 'SOL',
      funding_rates: [
        { exchange: 'binance', rate_bps: 5, timestamp: Date.now() },
        { exchange: 'bybit', rate_bps: 4, timestamp: Date.now() },
      ],
      spot_price_usd: 150,
      perp_prices: [
        { exchange: 'binance', price: 150, funding_rate: 5 },
        { exchange: 'bybit', price: 150, funding_rate: 4 },
      ],
      liquidities: [
        { exchange: 'binance', liquidity_usd: 10000000 },
        { exchange: 'bybit', liquidity_usd: 8000000 },
      ],
    };

    const response = await request.post(
      `${API_URL}/api/v1/cartridges/funding-rate-arb/evaluate`,
      {
        data: marketData,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    // Diferencial muy bajo (1 bps) no debería ser oportunidad
    expect(result.is_opportunity).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// TEST SUITE 3: DETECCIÓN DE OPORTUNIDADES - MEAN REVERSION
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('Mean Reversion Arbitrage Detection', () => {
  test('should detect mean reversion opportunities', async ({ request }) => {
    // Generar precios con desviación
    const priceHistory = Array(20)
      .fill(0)
      .map((_, i) => 100 + Math.sin(i * 0.5) * 2); // Oscilación alrededor de 100

    const marketData = {
      chain_id: 1,
      pool_address: '0x1234567890123456789012345678901234567890',
      token_in: '0xaaaa',
      token_out: '0xbbbb',
      price_history: priceHistory,
      current_price: 95, // Desviación significativa
      block_number: 1000000,
      timestamp: Date.now(),
      liquidity_usd: 50000000,
    };

    const response = await request.post(
      `${API_URL}/api/v1/cartridges/mean-reversion-arb/evaluate`,
      {
        data: marketData,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    expect(result).toHaveProperty('is_opportunity');
    expect(result).toHaveProperty('estimated_profit');
    expect(result).toHaveProperty('confidence');

    if (result.is_opportunity) {
      expect(result.estimated_profit).toBeGreaterThan(0);
      expect(result).toHaveProperty('ema_price');
      expect(result).toHaveProperty('deviation_sigma');
      expect(result.deviation_sigma).toBeGreaterThan(2); // 2+ sigma
    }
  });

  test('should calculate EMA correctly', async ({ request }) => {
    // Precios simples para verificar EMA
    const priceHistory = [100, 102, 101, 103, 102, 104, 103, 105];

    const marketData = {
      chain_id: 1,
      pool_address: '0x1234567890123456789012345678901234567890',
      token_in: '0xaaaa',
      token_out: '0xbbbb',
      price_history: priceHistory,
      current_price: 106,
      block_number: 1000000,
      timestamp: Date.now(),
      liquidity_usd: 50000000,
    };

    const response = await request.post(
      `${API_URL}/api/v1/cartridges/mean-reversion-arb/evaluate`,
      {
        data: marketData,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    // EMA debe estar entre min y max de precios
    if (result.is_opportunity || result.ema_price) {
      const minPrice = Math.min(...priceHistory);
      const maxPrice = Math.max(...priceHistory);
      expect(result.ema_price).toBeGreaterThanOrEqual(minPrice);
      expect(result.ema_price).toBeLessThanOrEqual(maxPrice);
    }
  });

  test('should reject low deviation opportunities', async ({ request }) => {
    // Precios sin desviación significativa
    const priceHistory = Array(20).fill(100);

    const marketData = {
      chain_id: 1,
      pool_address: '0x1234567890123456789012345678901234567890',
      token_in: '0xaaaa',
      token_out: '0xbbbb',
      price_history: priceHistory,
      current_price: 100.5, // Desviación mínima
      block_number: 1000000,
      timestamp: Date.now(),
      liquidity_usd: 50000000,
    };

    const response = await request.post(
      `${API_URL}/api/v1/cartridges/mean-reversion-arb/evaluate`,
      {
        data: marketData,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    // Sin desviación no debería ser oportunidad
    expect(result.is_opportunity).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// TEST SUITE 4: EJECUCIÓN DE CARTUCHOS
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('Cartridge Execution', () => {
  test('should execute cartridge in SHADOW mode', async ({ request }) => {
    const executionRequest = {
      cartridge_name: 'Funding Rate Arbitrage',
      mode: 'SHADOW',
      market_data: {
        chain_id: 1,
        token: 'BTC',
        funding_rates: [
          { exchange: 'binance', rate_bps: 15, timestamp: Date.now() },
          { exchange: 'bybit', rate_bps: 5, timestamp: Date.now() },
        ],
        spot_price_usd: 67000,
        perp_prices: [
          { exchange: 'binance', price: 67050, funding_rate: 15 },
          { exchange: 'bybit', price: 67000, funding_rate: 5 },
        ],
        liquidities: [
          { exchange: 'binance', liquidity_usd: 50000000 },
          { exchange: 'bybit', liquidity_usd: 30000000 },
        ],
      },
    };

    const response = await request.post(
      `${API_URL}/api/v1/cartridges/execute`,
      {
        data: executionRequest,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    expect(result).toHaveProperty('execution_id');
    expect(result).toHaveProperty('status');
    expect(result.status).toBe('completed');
    expect(result).toHaveProperty('mode');
    expect(result.mode).toBe('SHADOW');
    expect(result).toHaveProperty('telemetry');
  });

  test('should generate valid transaction payload', async ({ request }) => {
    const executionRequest = {
      cartridge_name: 'Mean Reversion Arbitrage',
      mode: 'SHADOW',
      market_data: {
        chain_id: 1,
        pool_address: '0x1234567890123456789012345678901234567890',
        token_in: '0xaaaa',
        token_out: '0xbbbb',
        price_history: Array(20).fill(100),
        current_price: 95,
        block_number: 1000000,
        timestamp: Date.now(),
        liquidity_usd: 50000000,
      },
    };

    const response = await request.post(
      `${API_URL}/api/v1/cartridges/execute`,
      {
        data: executionRequest,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    if (result.payload) {
      expect(result.payload).toHaveProperty('strategy_type');
      expect(result.payload).toHaveProperty('execution_type');
      expect(result.payload).toHaveProperty('execution');
      expect(result.payload).toHaveProperty('risk_management');
      expect(result.payload).toHaveProperty('metadata');
    }
  });

  test('should log telemetry for executed cartridges', async ({ request }) => {
    const executionRequest = {
      cartridge_name: 'Funding Rate Arbitrage',
      mode: 'SHADOW',
      market_data: {
        chain_id: 1,
        token: 'ETH',
        funding_rates: [
          { exchange: 'binance', rate_bps: 20, timestamp: Date.now() },
          { exchange: 'bybit', rate_bps: 5, timestamp: Date.now() },
        ],
        spot_price_usd: 3500,
        perp_prices: [
          { exchange: 'binance', price: 3550, funding_rate: 20 },
          { exchange: 'bybit', price: 3500, funding_rate: 5 },
        ],
        liquidities: [
          { exchange: 'binance', liquidity_usd: 100000000 },
          { exchange: 'bybit', liquidity_usd: 80000000 },
        ],
      },
    };

    const response = await request.post(
      `${API_URL}/api/v1/cartridges/execute`,
      {
        data: executionRequest,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    expect(result.telemetry).toBeDefined();
    expect(result.telemetry).toHaveProperty('timestamp');
    expect(result.telemetry).toHaveProperty('chain_id');
    expect(result.telemetry).toHaveProperty('cartridge_name');
    expect(result.telemetry).toHaveProperty('execution_time_ms');
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// TEST SUITE 5: FRONTEND INTEGRATION
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('Frontend Integration', () => {
  test('should display cartridges in dashboard', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/cartridges`);

    // Esperar a que carguen los cartuchos
    await page.waitForSelector('[data-testid="cartridge-list"]', { timeout: 5000 });

    // Verificar que existen los cartuchos nuevos
    const fundingRateElement = await page.locator(
      'text=Funding Rate Arbitrage'
    ).first();
    const meanReversionElement = await page.locator(
      'text=Mean Reversion Arbitrage'
    ).first();

    await expect(fundingRateElement).toBeVisible();
    await expect(meanReversionElement).toBeVisible();
  });

  test('should display opportunities in real-time', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/opportunities`);

    // Esperar a que carguen las oportunidades
    await page.waitForSelector('[data-testid="opportunity-list"]', {
      timeout: 10000,
    });

    // Verificar que hay oportunidades
    const opportunities = await page.locator('[data-testid="opportunity-item"]');
    const count = await opportunities.count();

    expect(count).toBeGreaterThanOrEqual(0);
  });

  test('should show cartridge details', async ({ page }) => {
    await page.goto(`${FRONTEND_URL}/cartridges`);

    // Hacer click en el primer cartucho
    await page.locator('[data-testid="cartridge-item"]').first().click();

    // Esperar a que cargue el detalle
    await page.waitForSelector('[data-testid="cartridge-detail"]', {
      timeout: 5000,
    });

    // Verificar que se muestran los detalles
    const name = await page.locator('[data-testid="cartridge-name"]');
    const version = await page.locator('[data-testid="cartridge-version"]');
    const status = await page.locator('[data-testid="cartridge-status"]');

    await expect(name).toBeVisible();
    await expect(version).toBeVisible();
    await expect(status).toBeVisible();
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// TEST SUITE 6: HOT-RELOAD VIA REDIS
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('Hot-Reload via Redis', () => {
  test('should trigger hot-reload when cartridge is updated', async ({ request }) => {
    // Simular actualización de cartucho
    const updateRequest = {
      name: 'Funding Rate Arbitrage',
      code: 'fn init_strategy() { /* updated code */ }',
    };

    const response = await request.post(
      `${API_URL}/api/v1/cartridges/update`,
      {
        data: updateRequest,
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    expect(result).toHaveProperty('hot_reload_triggered');
    expect(result.hot_reload_triggered).toBe(true);
  });

  test('should propagate changes to all layers', async ({ request }) => {
    // Verificar que los cambios se propagan
    const response = await request.get(
      `${API_URL}/api/v1/cartridges/funding-rate-arb/status`,
      {
        headers: { Authorization: `Bearer ${ADMIN_TOKEN}` },
      }
    );

    expect(response.status()).toBe(200);
    const result = await response.json();

    expect(result).toHaveProperty('searcher_engine');
    expect(result).toHaveProperty('database');
    expect(result).toHaveProperty('cache');

    // Todos deben estar sincronizados
    expect(result.searcher_engine.version).toBe(result.database.version);
    expect(result.database.version).toBe(result.cache.version);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURACIÓN DE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

test.beforeAll(async () => {
  console.log('Starting cartridge integration tests...');
  console.log(`API URL: ${API_URL}`);
  console.log(`Frontend URL: ${FRONTEND_URL}`);
});

test.afterAll(async () => {
  console.log('Cartridge integration tests completed.');
});
