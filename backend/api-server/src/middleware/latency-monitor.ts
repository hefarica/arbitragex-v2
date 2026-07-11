/**
 * LatencyMonitor middleware - tracks request latency for OMEGA Pipeline <100ms targets.
 *
 * Features:
 * - In-memory metrics storage (max 10000 entries)
 * - Automatic warning log when latency > 100ms
 * - Percentile and stats calculation per endpoint
 * - Non-blocking, minimal overhead
 */

import type { Request, Response, NextFunction } from "express";

interface LatencyMetric {
  path: string;
  method: string;
  latency_ms: number;
  timestamp: number;
  status_code: number;
}

interface LatencyStats {
  count: number;
  avg: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
  min: number;
}

export class LatencyMonitor {
  private metrics: LatencyMetric[] = [];
  private readonly maxMetrics = 10000;
  private readonly latencyThresholdMs = 100; // Target <100ms

  /**
   * Express middleware factory - returns middleware function
   */
  middleware() {
    return (req: Request, res: Response, next: NextFunction) => {
      const start = Date.now();

      res.on("finish", () => {
        const latency = Date.now() - start;

        this.metrics.push({
          path: req.path,
          method: req.method,
          latency_ms: latency,
          timestamp: Date.now(),
          status_code: res.statusCode,
        });

        // Limitar tamaño - mantener últimos maxMetrics
        if (this.metrics.length > this.maxMetrics) {
          this.metrics = this.metrics.slice(-this.maxMetrics);
        }

        // Log warning si excede target de 100ms
        if (latency > this.latencyThresholdMs) {
          console.warn(
            `[LATENCY] ${req.method} ${req.path} took ${latency}ms (>100ms target)`
          );
        }
      });

      next();
    };
  }

  /**
   * Get all metrics for a specific path
   */
  private getPathMetrics(path: string): LatencyMetric[] {
    return this.metrics.filter((m) => m.path === path);
  }

  /**
   * Calculate percentile for a specific path
   */
  getPercentile(path: string, percentile: number): number {
    const pathMetrics = this.getPathMetrics(path);
    if (pathMetrics.length === 0) return 0;

    const sorted = pathMetrics
      .map((m) => m.latency_ms)
      .sort((a, b) => a - b);
    const idx = Math.floor(sorted.length * (percentile / 100));
    return sorted[Math.min(idx, sorted.length - 1)] ?? 0;
  }

  /**
   * Get comprehensive stats for a specific path
   */
  getStats(path: string): LatencyStats | null {
    const pathMetrics = this.getPathMetrics(path);
    if (pathMetrics.length === 0) return null;

    const latencies = pathMetrics.map((m) => m.latency_ms);
    const sum = latencies.reduce((a, b) => a + b, 0);

    return {
      count: latencies.length,
      avg: sum / latencies.length,
      p50: this.getPercentile(path, 50),
      p95: this.getPercentile(path, 95),
      p99: this.getPercentile(path, 99),
      max: Math.max(...latencies),
      min: Math.min(...latencies),
    };
  }

  /**
   * Get all unique paths that have been monitored
   */
  getMonitoredPaths(): string[] {
    return [...new Set(this.metrics.map((m) => m.path))];
  }

  /**
   * Get summary stats for all paths
   */
  getAllStats(): Array<{ path: string; stats: LatencyStats }> {
    const paths = this.getMonitoredPaths();
    return paths
      .map((path) => {
        const stats = this.getStats(path);
        return stats ? { path, stats } : null;
      })
      .filter((item): item is { path: string; stats: LatencyStats } =>
        item !== null
      );
  }

  /**
   * Clear all metrics (useful for testing)
   */
  clear(): void {
    this.metrics = [];
  }

  /**
   * Get total number of metrics stored
   */
  getMetricCount(): number {
    return this.metrics.length;
  }
}

// Singleton instance for shared state across the application
let globalLatencyMonitor: LatencyMonitor | null = null;

export function getGlobalLatencyMonitor(): LatencyMonitor {
  if (!globalLatencyMonitor) {
    globalLatencyMonitor = new LatencyMonitor();
  }
  return globalLatencyMonitor;
}

export function resetGlobalLatencyMonitor(): void {
  globalLatencyMonitor = null;
}
