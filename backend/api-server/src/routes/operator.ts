/**
 * Routes /api/operator/*
 * ----------------------
 * - GET  /api/operator/me                  → identidad + preferencias + gates resueltos
 * - POST /api/operator/preferences         → actualiza ui_preferences (steward+)
 * - POST /api/operator/feature-overrides   → sovereign-only
 *
 * Sello C9.4 / C9.5 — 9-Layer Coherence: L8 + L9 obligatorios.
 */

import { Router } from 'express';
import type { Pool } from 'pg';
import { createHash } from 'crypto';
import {
  requireOperatorRole,
  buildOperatorAuditPayload,
} from '../middleware/operator-authz';

export function buildOperatorRouter(pool: Pool): Router {
  const router = Router();

  // -------------------------------------------------------------------------
  // GET /api/operator/me
  // -------------------------------------------------------------------------
  router.get('/me', requireOperatorRole('observer'), async (req, res) => {
    const op = req.operator!;
    const featureManifest = await pool.query(
      `SELECT feature_key, ui_path, backend_route, requires_layers, enabled, description
       FROM feature_manifest`
    );
    const manifest = featureManifest.rows.map(r => ({
      feature_key: r.feature_key,
      ui_path: r.ui_path,
      backend_route: r.backend_route,
      requires_layers: r.requires_layers,
      enabled: Boolean(r.enabled),
      description: r.description,
      // Aplica feature_overrides del operador
      effective_enabled:
        op.featureOverrides && typeof op.featureOverrides[r.feature_key] === 'boolean'
          ? (op.featureOverrides[r.feature_key] as boolean)
          : Boolean(r.enabled),
    }));

    res.json({
      operator: {
        operator_id: op.operatorId,
        display_name: op.displayName,
        signing_pubkey: op.signingPubkey,
        role: op.role,
        cap_usd_ceiling: op.capUsdCeiling,
        allowed_chains: op.allowedChains,
        allowed_registries: op.allowedRegistries,
        ui_preferences: op.uiPreferences,
        feature_overrides: op.featureOverrides,
        config_hash: op.configHash,
        enabled: op.enabled,
      },
      gates: {
        // Capacidades efectivas derivadas de role
        can_mutate_registries: op.role !== 'observer',
        can_escalate_capital: op.role === 'sovereign',
        can_promote_mainnet: op.role === 'sovereign',
        can_modify_feature_manifest: op.role === 'sovereign',
        can_view_audit: true,
        can_view_drift: true,
        can_trigger_hot_reload: op.role !== 'observer',
      },
      feature_manifest: manifest,
      coherence_rule: '9-Layer (Frontend→API→Handler→PG/Redis→Hot-reload→runtime_ack→Audit | +L8 Authz +L9 Operator Audit)',
    });
  });

  // -------------------------------------------------------------------------
  // POST /api/operator/preferences   (steward+)
  // -------------------------------------------------------------------------
  router.post('/preferences', requireOperatorRole('steward'), async (req, res) => {
    const op = req.operator!;
    const idempotencyKey = req.header('Idempotency-Key');
    if (!idempotencyKey) {
      res.status(400).json({ status: 'BLOCKED', reason: 'IDEMPOTENCY_KEY_REQUIRED' });
      return;
    }

    const body = req.body as { ui_preferences?: Record<string, unknown> };
    const newPrefs = body.ui_preferences ?? {};

    // Conserva tokens del tema (C9.1): theme sólo entre temas pre-existentes.
    const ALLOWED_THEMES = new Set(['system', 'light', 'dark']);
    if (typeof newPrefs.theme === 'string' && !ALLOWED_THEMES.has(newPrefs.theme as string)) {
      res.status(400).json({
        status: 'BLOCKED',
        reason: 'THEME_NOT_IN_PREEXISTING_SET',
        allowed: Array.from(ALLOWED_THEMES),
        layer: 'C9.1_STYLE_INVARIANCE',
      });
      return;
    }

    const configHashBefore = op.configHash;
    const newPayload = JSON.stringify(newPrefs);
    const configHashAfter = 'sha256:' + createHash('sha256').update(newPayload).digest('hex');

    const client = await pool.connect();
    try {
      await client.query('BEGIN');
      await client.query(
        `UPDATE operator_parametrization
         SET ui_preferences=$1, config_hash=$2, updated_at=NOW()
         WHERE operator_id=$3`,
        [newPrefs, configHashAfter, op.operatorId]
      );

      const audit = buildOperatorAuditPayload(req);
      await client.query(
        `INSERT INTO audit_event
          (action, entity_type, entity_id, idempotency_key,
           config_hash_before, config_hash_after,
           operator_id, operator_pubkey, operator_role,
           payload, created_at)
         VALUES ('operator.preferences.update','operator',$1,$2,$3,$4,$5,$6,$7,$8,NOW())
         ON CONFLICT (idempotency_key) DO NOTHING`,
        [
          op.operatorId,
          idempotencyKey,
          configHashBefore,
          configHashAfter,
          audit.operator_id,
          audit.operator_pubkey,
          audit.operator_role,
          newPrefs,
        ]
      );
      await client.query('COMMIT');
    } catch (err) {
      await client.query('ROLLBACK');
      throw err;
    } finally {
      client.release();
    }

    res.json({
      status: 'VERIFIED',
      operator_id: op.operatorId,
      config_hash_before: configHashBefore,
      config_hash_after: configHashAfter,
      layers_completed: ['api', 'handler', 'pg', 'authz', 'audit'],
    });
  });

  // -------------------------------------------------------------------------
  // POST /api/operator/feature-overrides   (sovereign-only)
  // -------------------------------------------------------------------------
  router.post('/feature-overrides', requireOperatorRole('sovereign'), async (req, res) => {
    const op = req.operator!;
    const idempotencyKey = req.header('Idempotency-Key');
    if (!idempotencyKey) {
      res.status(400).json({ status: 'BLOCKED', reason: 'IDEMPOTENCY_KEY_REQUIRED' });
      return;
    }

    const body = req.body as { feature_overrides?: Record<string, boolean> };
    const overrides = body.feature_overrides ?? {};

    const configHashBefore = op.configHash;
    const payload = JSON.stringify(overrides);
    const configHashAfter = 'sha256:' + createHash('sha256').update(payload).digest('hex');

    const client = await pool.connect();
    try {
      await client.query('BEGIN');
      await client.query(
        `UPDATE operator_parametrization
         SET feature_overrides=$1, config_hash=$2, updated_at=NOW()
         WHERE operator_id=$3`,
        [overrides, configHashAfter, op.operatorId]
      );

      const audit = buildOperatorAuditPayload(req);
      await client.query(
        `INSERT INTO audit_event
          (action, entity_type, entity_id, idempotency_key,
           config_hash_before, config_hash_after,
           operator_id, operator_pubkey, operator_role,
           payload, created_at)
         VALUES ('operator.feature_overrides.update','operator',$1,$2,$3,$4,$5,$6,$7,$8,NOW())
         ON CONFLICT (idempotency_key) DO NOTHING`,
        [
          op.operatorId,
          idempotencyKey,
          configHashBefore,
          configHashAfter,
          audit.operator_id,
          audit.operator_pubkey,
          audit.operator_role,
          overrides,
        ]
      );
      await client.query('COMMIT');
    } catch (err) {
      await client.query('ROLLBACK');
      throw err;
    } finally {
      client.release();
    }

    res.json({
      status: 'VERIFIED',
      operator_id: op.operatorId,
      config_hash_before: configHashBefore,
      config_hash_after: configHashAfter,
      layers_completed: ['api', 'handler', 'pg', 'authz', 'audit'],
    });
  });

  return router;
}
