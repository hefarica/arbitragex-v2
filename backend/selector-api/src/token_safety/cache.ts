/**
 * Token Safety Cache (G-TOK-1)
 * 
 * In-memory cache for token safety screening results.
 * Reduces API calls to external safety services.
 */

import type { TokenSafetyResult } from "./client.js";

export interface CacheEntry {
  result: TokenSafetyResult;
  expiresAt: number;
}

/**
 * Token Safety Cache
 * 
 * Provides caching for token safety screening results.
 */
export class TokenSafetyCache {
  private cache: Map<string, CacheEntry> = new Map();
  private ttlMs: number;

  constructor(ttlMs: number = 3600000) {
    this.ttlMs = ttlMs;
  }

  /**
   * Get a cached result
   */
  get(address: string, chainId: number): TokenSafetyResult | null {
    const key = this.getKey(address, chainId);
    const entry = this.cache.get(key);
    
    if (!entry) {
      return null;
    }
    
    // Check if expired
    if (Date.now() > entry.expiresAt) {
      this.cache.delete(key);
      return null;
    }
    
    return entry.result;
  }

  /**
   * Set a cached result
   */
  set(result: TokenSafetyResult): void {
    const key = this.getKey(result.address, result.chainId);
    const entry: CacheEntry = {
      result,
      expiresAt: Date.now() + this.ttlMs,
    };
    this.cache.set(key, entry);
  }

  /**
   * Delete a cached result
   */
  delete(address: string, chainId: number): boolean {
    const key = this.getKey(address, chainId);
    return this.cache.delete(key);
  }

  /**
   * Clear all cached results
   */
  clear(): void {
    this.cache.clear();
  }

  /**
   * Get cache statistics
   */
  getStats(): { size: number; ttlMs: number } {
    return {
      size: this.cache.size,
      ttlMs: this.ttlMs,
    };
  }

  /**
   * Prune expired entries
   */
  prune(): number {
    const now = Date.now();
    let pruned = 0;
    
    for (const [key, entry] of this.cache.entries()) {
      if (now > entry.expiresAt) {
        this.cache.delete(key);
        pruned++;
      }
    }
    
    return pruned;
  }

  private getKey(address: string, chainId: number): string {
    return `${chainId}:${address.toLowerCase()}`;
  }
}

// Singleton instance
export const tokenSafetyCache = new TokenSafetyCache();

export default TokenSafetyCache;
