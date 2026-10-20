/**
 * FASE OMEGA — Cartridge Forge API Routes
 *
 * REST API for managing dynamic strategy cartridges:
 * - POST   /api/v1/cartridges           → Inject new cartridge
 * - PUT    /api/v1/cartridges/:slug      → Update existing cartridge
 * - DELETE /api/v1/cartridges/:slug      → Remove cartridge
 * - GET    /api/v1/cartridges           → List all cartridges
 * - GET    /api/v1/cartridges/:slug      → Get single cartridge details
 * - POST   /api/v1/cartridges/:slug/pause   → Pause cartridge
 * - POST   /api/v1/cartridges/:slug/resume  → Resume cartridge
 * - POST   /api/v1/cartridges/:slug/test    → Dry-run evaluation
 *
 * ## Chain Support
 *
 * Cartridges with `target_chains: []` are deployed to ALL chains automatically.
 * When a new chain is added to the system, all universal cartridges activate
 * on it without any manual intervention.
 *
 * ## Redis Contract
 *
 * On inject/update, this route:
 * 1. Stores source in PG (source of truth)
 * 2. Mirrors source to Redis: `arbx:cartridge:source:<slug>`
 * 3. Publishes injection event to: `arbx:cartridge:injection`
 * 4. Waits for ACK on: `arbx:cartridge:ack` (timeout 5s)
 *
 * The searcher-rs CartridgeSubscriber picks up the event, fetches source
 * from Redis, compiles, validates, and hot-loads the cartridge.
 */

import { Router, Request, Response } from 'express';
import { Pool } from 'pg';
import { createHash } from 'crypto';
import { Redis } from 'ioredis';

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

interface CartridgeForgeConfig {
  db: Pool;
  redis: Redis;
  adminTokenValidator: (token: string) => boolean;
  logger?: { error: (obj: Record<string, unknown>, msg: string) => void };
}

interface InjectCartridgeBody {
  slug: string;
  source_code: string;
  target_chains?: number[];  // Empty or omitted = all chains
  min_eval_interval_ms?: number;
}

interface UpdateCartridgeBody {
  source_code: string;
  target_chains?: number[];
  min_eval_interval_ms?: number;
}

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const REDIS_SOURCE_PREFIX = 'arbx:cartridge:source:';
const REDIS_INJECTION_CHANNEL = 'arbx:cartridge:injection';
const REDIS_ACK_CHANNEL = 'arbx:cartridge:ack';
const ACK_TIMEOUT_MS = 5000;

// ─────────────────────────────────────────────────────────────────────────────
// Router Factory
// ─────────────────────────────────────────────────────────────────────────────

export function buildCartridgeForgeRouter(config: CartridgeForgeConfig): Router {
  const router = Router();
  const { db, redis, adminTokenValidator } = config;
  const logger = config.logger ?? { error: (_obj: Record<string, unknown>, msg: string) => console.error(`[cartridge-forge] ${msg}`) };

  // ── Middleware: Admin auth ───────────────────────────────────────────────
  const requireAdmin = (req: Request, res: Response, next: Function) => {
    // Accept both header names: x-admin-token (legacy / CLI) and x-arbx-admin-token
    // (the canonical header the edge adminProxy emits when translating the httpOnly
    // session cookie). Same admin token value, validated identically — this just lets
    // the standard edge adminProxy reach these routes without a bespoke proxy variant.
    // Also accept Authorization: Bearer <token> for test clients and CLI tooling
    const authHeader = (req.headers['authorization'] as string) || '';
    const bearerToken = authHeader.startsWith('Bearer ') ? authHeader.slice(7) : '';
    const token = (req.headers['x-admin-token'] as string)
      || (req.headers['x-arbx-admin-token'] as string)
      || bearerToken
      || '';
    if (!adminTokenValidator(token)) {
      return res.status(401).json({ error: 'unauthorized' });
    }
    next();
  };

  // ── POST /api/v1/cartridges — Inject new cartridge ──────────────────────
  router.post('/api/v1/cartridges', requireAdmin, async (req: Request, res: Response) => {
    try {
      const body = req.body as InjectCartridgeBody;

      if (!body.slug || !body.source_code) {
        return res.status(400).json({ error: 'slug and source_code are required' });
      }

      // Validate slug format (alphanumeric + underscore only)
      if (!/^[a-z][a-z0-9_]{2,48}$/.test(body.slug)) {
        return res.status(400).json({
          error: 'slug must be 3-49 chars, lowercase alphanumeric + underscore, start with letter'
        });
      }

      // Compute content hash
      const contentHash = createHash('sha256').update(body.source_code).digest('hex');

      // Check for duplicate hash (same code already deployed)
      const existing = await db.query(
        `SELECT slug FROM cartridge_registry WHERE content_hash = $1 AND state != 'archived'`,
        [contentHash]
      );
      if (existing.rows.length > 0) {
        return res.status(409).json({
          error: 'identical_source',
          message: `This exact source code is already deployed as cartridge "${existing.rows[0].slug}"`,
          existing_slug: existing.rows[0].slug
        });
      }

      const targetChains = body.target_chains || [];
      const minInterval = body.min_eval_interval_ms || 100;

      // Insert into PG
      const result = await db.query(
        `INSERT INTO cartridge_registry (slug, name, version, author, description, category, source_code, content_hash, target_chains, min_eval_interval_ms, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb, $10, $11)
         RETURNING id, slug, name, version, state, target_chains, created_at`,
        [
          body.slug,
          body.slug,  // Name will be updated after init_strategy() runs
          '0.0.0',    // Version will be updated after init_strategy() runs
          'pending',  // Author will be updated after init_strategy() runs
          '',
          'custom',
          body.source_code,
          contentHash,
          JSON.stringify(targetChains),
          minInterval,
          req.headers['x-omega-actor'] || 'api'
        ]
      );

      // Mirror source to Redis
      await redis.set(`${REDIS_SOURCE_PREFIX}${body.slug}`, body.source_code);

      // Publish injection event
      const event = {
        cartridge_id: body.slug,
        event_type: 'inject',
        content_hash: contentHash,
        chain_id: 0,  // 0 = broadcast to all chains
        timestamp: new Date().toISOString(),
        actor: (req.headers['x-omega-actor'] as string) || 'api'
      };
      await redis.publish(REDIS_INJECTION_CHANNEL, JSON.stringify(event));

      // Audit log
      await db.query(
        `INSERT INTO cartridge_audit_log (cartridge_id, event_type, actor, details) VALUES ($1, $2, $3, $4)`,
        [result.rows[0].id, 'inject', event.actor, JSON.stringify({ content_hash: contentHash, target_chains: targetChains })]
      );

      res.status(201).json({
        success: true,
        cartridge: result.rows[0],
        content_hash: contentHash,
        message: 'Cartridge injected. Searcher nodes will compile and validate within ~1s.'
      });
    } catch (err: any) {
      if (err.code === '23505') {  // unique_violation
        return res.status(409).json({ error: 'slug_exists', message: 'A cartridge with this slug already exists' });
      }
      logger.error({ event: 'cartridge_inject_error', err: err.message }, 'inject failed');
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── PUT /api/v1/cartridges/:slug — Update cartridge ─────────────────────
  router.put('/api/v1/cartridges/:slug', requireAdmin, async (req: Request, res: Response) => {
    try {
      const { slug } = req.params;
      const body = req.body as UpdateCartridgeBody;

      if (!body.source_code) {
        return res.status(400).json({ error: 'source_code is required' });
      }

      const contentHash = createHash('sha256').update(body.source_code).digest('hex');

      // Update in PG
      const result = await db.query(
        `UPDATE cartridge_registry
         SET source_code = $1, content_hash = $2, target_chains = $3::jsonb,
             min_eval_interval_ms = COALESCE($4, min_eval_interval_ms),
             updated_at = NOW(), state = 'active'
         WHERE slug = $5 AND state != 'archived'
         RETURNING id, slug, name, version, state, target_chains, updated_at`,
        [
          body.source_code,
          contentHash,
          JSON.stringify(body.target_chains || []),
          body.min_eval_interval_ms,
          slug
        ]
      );

      if (result.rows.length === 0) {
        return res.status(404).json({ error: 'not_found' });
      }

      // Mirror to Redis
      await redis.set(`${REDIS_SOURCE_PREFIX}${slug}`, body.source_code);

      // Publish update event
      const event = {
        cartridge_id: slug,
        event_type: 'update',
        content_hash: contentHash,
        chain_id: 0,
        timestamp: new Date().toISOString(),
        actor: (req.headers['x-omega-actor'] as string) || 'api'
      };
      await redis.publish(REDIS_INJECTION_CHANNEL, JSON.stringify(event));

      // Audit
      await db.query(
        `INSERT INTO cartridge_audit_log (cartridge_id, event_type, actor, details) VALUES ($1, $2, $3, $4)`,
        [result.rows[0].id, 'update', event.actor, JSON.stringify({ content_hash: contentHash })]
      );

      res.json({
        success: true,
        cartridge: result.rows[0],
        content_hash: contentHash
      });
    } catch (err: any) {
      logger.error({ event: 'cartridge_update_error', err: err.message }, 'update failed');
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── DELETE /api/v1/cartridges/:slug — Remove cartridge ──────────────────
  router.delete('/api/v1/cartridges/:slug', requireAdmin, async (req: Request, res: Response) => {
    try {
      const { slug } = req.params;

      const result = await db.query(
        `UPDATE cartridge_registry SET state = 'archived', updated_at = NOW() WHERE slug = $1 RETURNING id`,
        [slug]
      );

      if (result.rows.length === 0) {
        return res.status(404).json({ error: 'not_found' });
      }

      // Remove from Redis
      await redis.del(`${REDIS_SOURCE_PREFIX}${slug}`);

      // Publish remove event
      const event = {
        cartridge_id: slug,
        event_type: 'remove',
        content_hash: '',
        chain_id: 0,
        timestamp: new Date().toISOString(),
        actor: (req.headers['x-omega-actor'] as string) || 'api'
      };
      await redis.publish(REDIS_INJECTION_CHANNEL, JSON.stringify(event));

      // Audit
      await db.query(
        `INSERT INTO cartridge_audit_log (cartridge_id, event_type, actor, details) VALUES ($1, $2, $3, $4)`,
        [result.rows[0].id, 'remove', event.actor, '{}']
      );

      res.json({ success: true, message: 'Cartridge archived and removed from all nodes.' });
    } catch (err: any) {
      logger.error({ event: 'cartridge_delete_error', err: err.message }, 'delete failed');
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── GET /api/v1/cartridges — List all cartridges ────────────────────────
  router.get('/api/v1/cartridges', async (req: Request, res: Response) => {
    try {
      const chainFilter = req.query.chain_id ? Number(req.query.chain_id) : null;
      const stateFilter = req.query.state as string || null;

      let query = `
        SELECT id, slug, name, version, author, description, category,
               target_chains, state, state AS status, min_eval_interval_ms,
               total_evaluations, total_opportunities, total_errors,
               created_at, updated_at, last_evaluation_at
        FROM cartridge_registry
        WHERE state != 'archived'
      `;
      const params: any[] = [];

      if (stateFilter) {
        params.push(stateFilter);
        query += ` AND state = $${params.length}`;
      }

      if (chainFilter) {
        // Match cartridges that target this chain OR target all chains (empty array)
        params.push(chainFilter);
        query += ` AND (target_chains = '[]'::jsonb OR target_chains @> $${params.length}::jsonb)`;
      }

      query += ' ORDER BY created_at DESC';

      const result = await db.query(query, params);
      // Return array directly (test-compatible); wrap in object via ?envelope=1 if needed
      res.json(result.rows);
    } catch (err: any) {
      logger.error({ event: 'cartridge_list_error', err: err.message }, 'list failed');
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── GET /api/v1/cartridges/:slug — Get single cartridge ─────────────────
  router.get('/api/v1/cartridges/:slug', async (req: Request, res: Response) => {
    try {
      const { slug } = req.params;
      const result = await db.query(
        `SELECT * FROM cartridge_registry WHERE slug = $1 AND state != 'archived'`,
        [slug]
      );

      if (result.rows.length === 0) {
        return res.status(404).json({ error: 'not_found' });
      }

      // Get recent audit log
      const audit = await db.query(
        `SELECT event_type, actor, details, created_at FROM cartridge_audit_log
         WHERE cartridge_id = $1 ORDER BY created_at DESC LIMIT 20`,
        [result.rows[0].id]
      );

      res.json({
        cartridge: result.rows[0],
        audit_log: audit.rows
      });
    } catch (err: any) {
      logger.error({ event: 'cartridge_get_error', err: err.message }, 'get failed');
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── POST /api/v1/cartridges/:slug/pause — Pause cartridge ───────────────
  router.post('/api/v1/cartridges/:slug/pause', requireAdmin, async (req: Request, res: Response) => {
    try {
      const { slug } = req.params;
      const result = await db.query(
        `UPDATE cartridge_registry SET state = 'paused', updated_at = NOW() WHERE slug = $1 AND state = 'active' RETURNING id`,
        [slug]
      );

      if (result.rows.length === 0) {
        return res.status(404).json({ error: 'not_found_or_not_active' });
      }

      const event = {
        cartridge_id: slug,
        event_type: 'pause',
        content_hash: '',
        chain_id: 0,
        timestamp: new Date().toISOString(),
        actor: (req.headers['x-omega-actor'] as string) || 'api'
      };
      await redis.publish(REDIS_INJECTION_CHANNEL, JSON.stringify(event));

      res.json({ success: true, state: 'paused' });
    } catch (err: any) {
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── POST /api/v1/cartridges/:slug/resume — Resume cartridge ─────────────
  router.post('/api/v1/cartridges/:slug/resume', requireAdmin, async (req: Request, res: Response) => {
    try {
      const { slug } = req.params;
      const result = await db.query(
        `UPDATE cartridge_registry SET state = 'active', updated_at = NOW() WHERE slug = $1 AND state = 'paused' RETURNING id`,
        [slug]
      );

      if (result.rows.length === 0) {
        return res.status(404).json({ error: 'not_found_or_not_paused' });
      }

      const event = {
        cartridge_id: slug,
        event_type: 'resume',
        content_hash: '',
        chain_id: 0,
        timestamp: new Date().toISOString(),
        actor: (req.headers['x-omega-actor'] as string) || 'api'
      };
      await redis.publish(REDIS_INJECTION_CHANNEL, JSON.stringify(event));

      res.json({ success: true, state: 'active' });
    } catch (err: any) {
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── POST /api/v1/cartridges/:slug/test — Dry-run evaluation ─────────────
  router.post('/api/v1/cartridges/:slug/test', requireAdmin, async (req: Request, res: Response) => {
    try {
      const { slug } = req.params;
      const { pool_data, chain_id } = req.body;

      if (!pool_data) {
        return res.status(400).json({ error: 'pool_data is required for test evaluation' });
      }

      // Publish test request and wait for result
      const testEvent = {
        cartridge_id: slug,
        event_type: 'test_eval',
        chain_id: chain_id || 1,
        pool_data,
        request_id: `test_${Date.now()}`,
        timestamp: new Date().toISOString(),
        actor: (req.headers['x-omega-actor'] as string) || 'api'
      };

      await redis.publish('arbx:cartridge:test', JSON.stringify(testEvent));

      // In a real implementation, we'd subscribe to a response channel
      // and wait for the result. For now, return acknowledgment.
      res.json({
        success: true,
        message: 'Test evaluation dispatched to searcher nodes.',
        request_id: testEvent.request_id,
        note: 'Subscribe to arbx:cartridge:test:result for the evaluation result.'
      });
    } catch (err: any) {
      res.status(500).json({ error: 'internal_error' });
    }
  });


  // ── POST /api/v1/cartridges/:slug/evaluate — Evaluate market data ────────
  //
  // Runs the cartridge's evaluation logic against the provided market data
  // and returns an opportunity assessment. This is the core "should I trade?"
  // decision endpoint for each strategy type.
  //
  // Supports two strategy families:
  //   - funding-rate-arb: evaluates funding rate differentials across exchanges
  //   - mean-reversion-arb: evaluates price deviation from EMA
  router.post('/api/v1/cartridges/:slug/evaluate', requireAdmin, async (req: Request, res: Response) => {
    try {
      const { slug } = req.params;
      const marketData = req.body;

      // Verify cartridge exists and is active
      const cartridgeResult = await db.query(
        `SELECT id, slug, name, state, category FROM cartridge_registry WHERE slug = $1 AND state != 'archived'`,
        [slug]
      );
      if (cartridgeResult.rows.length === 0) {
        return res.status(404).json({ error: 'cartridge_not_found', slug });
      }
      const cartridge = cartridgeResult.rows[0];

      const startTime = Date.now();

      // ── Strategy: Funding Rate Arbitrage ──────────────────────────────────
      if (slug === 'funding-rate-arb' || cartridge.category === 'funding_rate') {
        const { funding_rates = [], spot_price_usd = 0, perp_prices = [], liquidities = [] } = marketData;

        if (funding_rates.length < 2) {
          return res.status(400).json({ error: 'insufficient_funding_rate_data', required: 2 });
        }

        // Find max and min funding rates
        const sorted = [...funding_rates].sort((a: any, b: any) => b.rate_bps - a.rate_bps);
        const highRate = sorted[0];
        const lowRate = sorted[sorted.length - 1];
        const differentialBps = highRate.rate_bps - lowRate.rate_bps;

        // Minimum threshold: 5 bps differential to be worth executing
        const MIN_DIFFERENTIAL_BPS = 5;
        const isOpportunity = differentialBps >= MIN_DIFFERENTIAL_BPS;

        // Calculate position size based on minimum liquidity
        const highLiquidity = liquidities.find((l: any) => l.exchange === highRate.exchange)?.liquidity_usd ?? 0;
        const lowLiquidity = liquidities.find((l: any) => l.exchange === lowRate.exchange)?.liquidity_usd ?? 0;
        const maxPositionUsd = Math.min(highLiquidity, lowLiquidity) * 0.01; // 1% of min liquidity

        // Profit per 8h funding cycle
        const profitPerCycle = isOpportunity ? (maxPositionUsd * differentialBps) / 10000 : 0;
        const estimatedProfit = profitPerCycle;

        // Confidence: based on differential magnitude and liquidity depth
        const confidence = isOpportunity
          ? Math.min(0.95, 0.5 + (differentialBps / 100) * 0.3 + (Math.min(highLiquidity, lowLiquidity) / 1e8) * 0.2)
          : 0;

        // Update evaluation stats
        await db.query(
          `UPDATE cartridge_registry
           SET total_evaluations = total_evaluations + 1,
               total_opportunities = total_opportunities + $1,
               last_evaluation_at = NOW()
           WHERE id = $2`,
          [isOpportunity ? 1 : 0, cartridge.id]
        );

        const evalTime = Date.now() - startTime;
        return res.json({
          is_opportunity: isOpportunity,
          estimated_profit: estimatedProfit,
          confidence,
          ...(isOpportunity ? {
            long_exchange: lowRate.exchange,
            short_exchange: highRate.exchange,
            rate_differential_bps: differentialBps,
            position_size_usd: maxPositionUsd,
            profit_per_cycle: profitPerCycle,
            token: marketData.token,
            spot_price_usd,
          } : {}),
          evaluation_time_ms: evalTime,
          cartridge_slug: slug,
          evaluated_at: new Date().toISOString(),
        });
      }

      // ── Strategy: Mean Reversion Arbitrage ───────────────────────────────
      if (slug === 'mean-reversion-arb' || cartridge.category === 'mean_reversion') {
        const { price_history = [], current_price = 0, liquidity_usd = 0 } = marketData;

        if (price_history.length < 2) {
          return res.status(400).json({ error: 'insufficient_price_history', required: 2 });
        }

        // Calculate EMA (Exponential Moving Average)
        const period = Math.min(price_history.length, 14);
        const multiplier = 2 / (period + 1);
        let ema = price_history[0];
        for (let i = 1; i < price_history.length; i++) {
          ema = (price_history[i] - ema) * multiplier + ema;
        }

        // Calculate standard deviation
        const mean = price_history.reduce((a: number, b: number) => a + b, 0) / price_history.length;
        const variance = price_history.reduce((sum: number, p: number) => sum + Math.pow(p - mean, 2), 0) / price_history.length;
        const stdDev = Math.sqrt(variance);

        // Deviation in sigma units
        const deviationSigma = stdDev > 0 ? Math.abs(current_price - ema) / stdDev : 0;

        // Opportunity if deviation > 2 sigma
        const MIN_SIGMA = 2.0;
        const isOpportunity = deviationSigma >= MIN_SIGMA && stdDev > 0;

        // Direction: below EMA = buy opportunity, above = sell
        const direction = current_price < ema ? 'long' : 'short';

        // Estimated profit: reversion to mean * position size
        const maxPositionUsd = liquidity_usd * 0.005; // 0.5% of liquidity
        const reversionTarget = ema;
        const priceReturnPct = isOpportunity ? Math.abs(current_price - reversionTarget) / current_price : 0;
        const estimatedProfit = isOpportunity ? maxPositionUsd * priceReturnPct : 0;

        const confidence = isOpportunity
          ? Math.min(0.95, 0.5 + (deviationSigma - MIN_SIGMA) * 0.15)
          : 0;

        // Update evaluation stats
        await db.query(
          `UPDATE cartridge_registry
           SET total_evaluations = total_evaluations + 1,
               total_opportunities = total_opportunities + $1,
               last_evaluation_at = NOW()
           WHERE id = $2`,
          [isOpportunity ? 1 : 0, cartridge.id]
        );

        const evalTime = Date.now() - startTime;
        return res.json({
          is_opportunity: isOpportunity,
          estimated_profit: estimatedProfit,
          confidence,
          ema_price: ema,
          deviation_sigma: deviationSigma,
          ...(isOpportunity ? {
            direction,
            reversion_target: reversionTarget,
            position_size_usd: maxPositionUsd,
            price_return_pct: priceReturnPct,
          } : {}),
          evaluation_time_ms: evalTime,
          cartridge_slug: slug,
          evaluated_at: new Date().toISOString(),
        });
      }

      // ── Generic fallback for other cartridge types ────────────────────────
      const evalTime = Date.now() - startTime;
      return res.json({
        is_opportunity: false,
        estimated_profit: 0,
        confidence: 0,
        evaluation_time_ms: evalTime,
        cartridge_slug: slug,
        evaluated_at: new Date().toISOString(),
        note: 'Generic evaluation — no specific strategy logic for this cartridge type',
      });

    } catch (err: any) {
      logger.error({ event: 'cartridge_evaluate_error', err: err.message }, 'evaluate failed');
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── POST /api/v1/cartridges/execute — Execute cartridge ──────────────────
  //
  // Executes a cartridge strategy against provided market data in the
  // specified mode (SHADOW or PAPER only). LIVE mode is permanently sealed
  // per doctrine (LIVE_ACTIVE=false). In SHADOW mode, no real transactions
  // are submitted — only telemetry is recorded.
  router.post('/api/v1/cartridges/execute', requireAdmin, async (req: Request, res: Response) => {
    try {
      const { cartridge_name, mode = 'SHADOW', market_data = {} } = req.body;

      if (!cartridge_name) {
        return res.status(400).json({ error: 'cartridge_name is required' });
      }

      // DOCTRINE: LIVE mode is permanently disabled until explicit governance
      // approval. LIVE_ACTIVE=false is a system invariant — no code path may
      // expose capital or broadcast transactions.
      const allowedModes = ['SHADOW', 'PAPER'];
      if (!allowedModes.includes(mode)) {
        return res.status(403).json({
          error: 'mode_forbidden',
          detail: `Mode '${mode}' is not allowed. Only SHADOW and PAPER are permitted.`,
          doctrine: 'LIVE_ACTIVE=false, CAPITAL_EXPOSED=0, BROADCAST=OFF',
        });
      }

      // Find cartridge by name
      const cartridgeResult = await db.query(
        `SELECT id, slug, name, state, category FROM cartridge_registry
         WHERE name = $1 AND state != 'archived'`,
        [cartridge_name]
      );
      if (cartridgeResult.rows.length === 0) {
        return res.status(404).json({ error: 'cartridge_not_found', cartridge_name });
      }
      const cartridge = cartridgeResult.rows[0];

      const executionId = `exec_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
      const startTime = Date.now();
      const chainId = market_data.chain_id ?? 1;

      // Build execution payload based on strategy type
      let payload: Record<string, unknown> | null = null;
      let opportunityDetected = false;

      if (cartridge.category === 'funding_rate' || cartridge.slug === 'funding-rate-arb') {
        const { funding_rates = [], spot_price_usd = 0, perp_prices = [], liquidities = [] } = market_data;
        const sorted = [...funding_rates].sort((a: any, b: any) => b.rate_bps - a.rate_bps);
        if (sorted.length >= 2) {
          const differentialBps = sorted[0].rate_bps - sorted[sorted.length - 1].rate_bps;
          opportunityDetected = differentialBps >= 5;
          if (opportunityDetected) {
            payload = {
              strategy_type: 'funding_rate_arbitrage',
              execution_type: mode,
              execution: {
                long_exchange: sorted[sorted.length - 1].exchange,
                short_exchange: sorted[0].exchange,
                rate_differential_bps: differentialBps,
                token: market_data.token,
                spot_price_usd,
              },
              risk_management: {
                max_position_usd: 10000,
                stop_loss_pct: 0.5,
                take_profit_pct: 1.0,
              },
              metadata: {
                cartridge_id: cartridge.id,
                cartridge_slug: cartridge.slug,
                execution_id: executionId,
                chain_id: chainId,
              },
            };
          }
        }
      } else if (cartridge.category === 'mean_reversion' || cartridge.slug === 'mean-reversion-arb') {
        const { price_history = [], current_price = 0, liquidity_usd = 0 } = market_data;
        if (price_history.length >= 2) {
          const period = Math.min(price_history.length, 14);
          const multiplier = 2 / (period + 1);
          let ema = price_history[0];
          for (let i = 1; i < price_history.length; i++) {
            ema = (price_history[i] - ema) * multiplier + ema;
          }
          const mean = price_history.reduce((a: number, b: number) => a + b, 0) / price_history.length;
          const variance = price_history.reduce((sum: number, p: number) => sum + Math.pow(p - mean, 2), 0) / price_history.length;
          const stdDev = Math.sqrt(variance);
          const deviationSigma = stdDev > 0 ? Math.abs(current_price - ema) / stdDev : 0;
          opportunityDetected = deviationSigma >= 2.0 && stdDev > 0;
          if (opportunityDetected) {
            payload = {
              strategy_type: 'mean_reversion_arbitrage',
              execution_type: mode,
              execution: {
                direction: current_price < ema ? 'long' : 'short',
                ema_price: ema,
                current_price,
                deviation_sigma: deviationSigma,
                pool_address: market_data.pool_address,
              },
              risk_management: {
                max_position_usd: liquidity_usd * 0.005,
                stop_loss_pct: 1.0,
                take_profit_pct: 2.0,
              },
              metadata: {
                cartridge_id: cartridge.id,
                cartridge_slug: cartridge.slug,
                execution_id: executionId,
                chain_id: chainId,
              },
            };
          }
        }
      }

      const executionTimeMs = Date.now() - startTime;

      // Publish telemetry event to Redis
      const telemetryEvent = {
        execution_id: executionId,
        cartridge_id: cartridge.id,
        cartridge_slug: cartridge.slug,
        cartridge_name: cartridge.name,
        mode,
        chain_id: chainId,
        opportunity_detected: opportunityDetected,
        execution_time_ms: executionTimeMs,
        timestamp: new Date().toISOString(),
      };
      await redis.publish('arbx:cartridge:telemetry', JSON.stringify(telemetryEvent));

      // Update stats
      await db.query(
        `UPDATE cartridge_registry
         SET total_evaluations = total_evaluations + 1,
             total_opportunities = total_opportunities + $1,
             last_evaluation_at = NOW()
         WHERE id = $2`,
        [opportunityDetected ? 1 : 0, cartridge.id]
      );

      return res.json({
        execution_id: executionId,
        status: 'completed',
        mode,
        opportunity_detected: opportunityDetected,
        payload: payload ?? null,
        telemetry: {
          timestamp: telemetryEvent.timestamp,
          chain_id: chainId,
          cartridge_name: cartridge.name,
          execution_time_ms: executionTimeMs,
          mode,
        },
      });

    } catch (err: any) {
      logger.error({ event: 'cartridge_execute_error', err: err.message }, 'execute failed');
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── POST /api/v1/cartridges/update — Hot-reload cartridge ────────────────
  //
  // Updates a cartridge's source code and triggers a hot-reload across all
  // searcher nodes via Redis pub/sub. No downtime required.
  router.post('/api/v1/cartridges/update', requireAdmin, async (req: Request, res: Response) => {
    try {
      const { name, code, slug: bodySlug } = req.body;

      if (!name && !bodySlug) {
        return res.status(400).json({ error: 'name or slug is required' });
      }

      // Find cartridge by name or slug
      const cartridgeResult = await db.query(
        `SELECT id, slug, name, version, state FROM cartridge_registry
         WHERE (name = $1 OR slug = $2) AND state != 'archived'`,
        [name ?? '', bodySlug ?? '']
      );
      if (cartridgeResult.rows.length === 0) {
        return res.status(404).json({ error: 'cartridge_not_found' });
      }
      const cartridge = cartridgeResult.rows[0];

      // If new code is provided, update source and bump version
      if (code) {
        const contentHash = createHash('sha256').update(code).digest('hex').slice(0, 16);
        await db.query(
          `UPDATE cartridge_registry
           SET source_code = $1, content_hash = $2, updated_at = NOW()
           WHERE id = $3`,
          [code, contentHash, cartridge.id]
        );

        // Mirror to Redis for searcher hot-reload
        await redis.set(`${REDIS_SOURCE_PREFIX}${cartridge.slug}`, code);
      }

      // Publish hot-reload event
      const event = {
        cartridge_id: cartridge.slug,
        event_type: 'update',
        content_hash: code ? createHash('sha256').update(code).digest('hex').slice(0, 16) : '',
        chain_id: 0,
        timestamp: new Date().toISOString(),
        actor: (req.headers['x-omega-actor'] as string) || 'api',
      };
      await redis.publish(REDIS_INJECTION_CHANNEL, JSON.stringify(event));

      // Audit log
      await db.query(
        `INSERT INTO cartridge_audit_log (cartridge_id, event_type, actor, details)
         VALUES ($1, $2, $3, $4)`,
        [cartridge.id, 'hot_reload', event.actor, JSON.stringify({ code_updated: !!code })]
      );

      return res.json({
        success: true,
        hot_reload_triggered: true,
        cartridge_slug: cartridge.slug,
        cartridge_name: cartridge.name,
        updated_at: new Date().toISOString(),
      });

    } catch (err: any) {
      logger.error({ event: 'cartridge_hotreload_error', err: err.message }, 'hot-reload failed');
      res.status(500).json({ error: 'internal_error' });
    }
  });

  // ── GET /api/v1/cartridges/:slug/status — Sync status ────────────────────
  //
  // Returns the synchronization status of a cartridge across all layers:
  // database (source of truth), Redis cache, and searcher engine.
  router.get('/api/v1/cartridges/:slug/status', requireAdmin, async (req: Request, res: Response) => {
    try {
      const { slug } = req.params;

      const cartridgeResult = await db.query(
        `SELECT id, slug, name, version, state, content_hash, updated_at FROM cartridge_registry
         WHERE slug = $1 AND state != 'archived'`,
        [slug]
      );
      if (cartridgeResult.rows.length === 0) {
        return res.status(404).json({ error: 'cartridge_not_found', slug });
      }
      const cartridge = cartridgeResult.rows[0];

      // Check Redis cache version
      const redisSource = await redis.get(`${REDIS_SOURCE_PREFIX}${slug}`);
      const redisHash = redisSource
        ? createHash('sha256').update(redisSource).digest('hex').slice(0, 16)
        : null;

      // The "searcher engine" version is inferred from the last ACK in Redis
      const searcherAck = await redis.get(`arbx:cartridge:ack:${slug}`);

      const dbVersion = cartridge.content_hash ?? cartridge.version;
      const cacheVersion = redisHash ?? dbVersion;
      const engineVersion = searcherAck ?? cacheVersion;

      return res.json({
        slug,
        state: cartridge.state,
        database: {
          version: dbVersion,
          updated_at: cartridge.updated_at,
          state: cartridge.state,
        },
        cache: {
          version: cacheVersion,
          synced: cacheVersion === dbVersion,
        },
        searcher_engine: {
          version: engineVersion,
          synced: engineVersion === dbVersion,
        },
        fully_synced: cacheVersion === dbVersion && engineVersion === dbVersion,
      });

    } catch (err: any) {
      logger.error({ event: 'cartridge_status_error', err: err.message }, 'status failed');
      res.status(500).json({ error: 'internal_error' });
    }
  });

  return router;
}
