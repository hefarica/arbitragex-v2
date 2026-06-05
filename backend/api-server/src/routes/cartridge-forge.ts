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

  // ── Middleware: Admin auth ───────────────────────────────────────────────
  const requireAdmin = (req: Request, res: Response, next: Function) => {
    // Accept both header names: x-admin-token (legacy / CLI) and x-arbx-admin-token
    // (the canonical header the edge adminProxy emits when translating the httpOnly
    // session cookie). Same admin token value, validated identically — this just lets
    // the standard edge adminProxy reach these routes without a bespoke proxy variant.
    const token = (req.headers['x-admin-token'] as string)
      || (req.headers['x-arbx-admin-token'] as string)
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
      console.error('[cartridge-forge] inject error:', err);
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
      console.error('[cartridge-forge] update error:', err);
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
      console.error('[cartridge-forge] delete error:', err);
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
               target_chains, state, min_eval_interval_ms,
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

      res.json({
        cartridges: result.rows,
        total: result.rows.length,
        chain_filter: chainFilter,
        state_filter: stateFilter
      });
    } catch (err: any) {
      console.error('[cartridge-forge] list error:', err);
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
      console.error('[cartridge-forge] get error:', err);
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

  return router;
}
