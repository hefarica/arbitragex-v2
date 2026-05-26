/**
 * Token Blacklist Policy (G-TOK-1)
 * 
 * Maintains a list of blacklisted tokens that should be excluded from
 * arbitrage operations due to security concerns (honeypots, scams, etc.)
 */

export interface BlacklistEntry {
  address: string;
  chainId: number;
  reason: BlacklistReason;
  addedAt: Date;
  addedBy: string;
  evidence?: string;
}

export enum BlacklistReason {
  HONEYPOT = "honeypot",
  HIGH_TAX = "high_tax",
  SCAM = "scam",
  FAKE_TOKEN = "fake_token",
  EXPLOIT = "exploit",
  MANUAL = "manual",
}

export interface BlacklistFilterResult {
  isBlacklisted: boolean;
  reason?: BlacklistReason;
  entry?: BlacklistEntry;
}

/**
 * Token Blacklist Manager
 * 
 * Provides methods to check and manage the token blacklist.
 */
export class TokenBlacklist {
  private blacklist: Map<string, BlacklistEntry> = new Map();

  /**
   * Check if a token is blacklisted
   */
  isBlacklisted(address: string, chainId: number): BlacklistFilterResult {
    const key = this.getKey(address, chainId);
    const entry = this.blacklist.get(key);
    
    if (entry) {
      return {
        isBlacklisted: true,
        reason: entry.reason,
        entry,
      };
    }
    
    return { isBlacklisted: false };
  }

  /**
   * Add a token to the blacklist
   */
  add(entry: BlacklistEntry): void {
    const key = this.getKey(entry.address, entry.chainId);
    this.blacklist.set(key, entry);
  }

  /**
   * Remove a token from the blacklist
   */
  remove(address: string, chainId: number): boolean {
    const key = this.getKey(address, chainId);
    return this.blacklist.delete(key);
  }

  /**
   * Get all blacklisted tokens for a chain
   */
  getByChain(chainId: number): BlacklistEntry[] {
    return Array.from(this.blacklist.values())
      .filter(entry => entry.chainId === chainId);
  }

  /**
   * Get total count of blacklisted tokens
   */
  get size(): number {
    return this.blacklist.size;
  }

  private getKey(address: string, chainId: number): string {
    return `${chainId}:${address.toLowerCase()}`;
  }
}

// Singleton instance
export const tokenBlacklist = new TokenBlacklist();

/**
 * Filter tokens against the blacklist
 */
export function filterBlacklistedTokens(
  addresses: string[],
  chainId: number,
): { allowed: string[]; blocked: BlacklistFilterResult[] } {
  const allowed: string[] = [];
  const blocked: BlacklistFilterResult[] = [];

  for (const address of addresses) {
    const result = tokenBlacklist.isBlacklisted(address, chainId);
    if (result.isBlacklisted) {
      blocked.push(result);
    } else {
      allowed.push(address);
    }
  }

  return { allowed, blocked };
}

export default TokenBlacklist;
