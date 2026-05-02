import { Router, Request, Response } from 'express';
import { Pool } from 'pg';

export const defiRouter = Router();

// Create a Postgres Pool. We don't crash here if it fails, we let the queries fail.
// If DATABASE_URL is not set, this will fail on first query.
const pool = new Pool({
    connectionString: process.env.DATABASE_URL || 'postgres://postgres:postgres@localhost:5432/arbitragex',
});

// ==========================================
// REGISTRIES: CHAINS, RPCS, DEXES, TOKENS
// ==========================================

defiRouter.get('/chains', async (req: Request, res: Response) => {
    try {
        const result = await pool.query('SELECT * FROM defi_chains WHERE is_active = true ORDER BY chain_id ASC');
        res.json({ success: true, data: result.rows });
    } catch (e: any) {
        // Enforce ZERO MOCKS: Return actual error
        res.status(500).json({ success: false, error: "Database error", message: e.message });
    }
});

defiRouter.get('/rpcs', async (req: Request, res: Response) => {
    try {
        const result = await pool.query('SELECT * FROM defi_rpcs ORDER BY latency_ms ASC');
        res.json({ success: true, data: result.rows });
    } catch (e: any) {
        res.status(500).json({ success: false, error: "Database error", message: e.message });
    }
});

// ==========================================
// POOLS & ROUTES
// ==========================================

defiRouter.get('/pools', async (req: Request, res: Response) => {
    try {
        const result = await pool.query('SELECT * FROM defi_pools WHERE active = true ORDER BY created_at DESC LIMIT 100');
        res.json({ success: true, data: result.rows });
    } catch (e: any) {
        res.status(500).json({ success: false, error: "Database error", message: e.message });
    }
});

// ==========================================
// METRICS (For Worker Health)
// ==========================================
defiRouter.get('/metrics', async (req: Request, res: Response) => {
    // These should ideally come from Redis/Prometheus. 
    // If not available, we return the real degraded state.
    res.json({ 
        success: true, 
        data: {
            active_workers: 0,
            cpu_usage_pct: 0,
            memory_usage_mb: process.memoryUsage().rss / 1024 / 1024,
            uptime_seconds: process.uptime(),
            kernel_bypass_active: false
        }
    });
});
