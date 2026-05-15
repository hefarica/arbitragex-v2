/**
 * SSR snapshot loader for /omega-s5/registry/[entity].
 *
 * R8 doctrine: returns a discriminated union instead of throwing. Pages
 * always render HTTP 200; the client uses the status to choose between
 * AUTH_REQUIRED, UNAVAILABLE, and OK presentations.
 */
import { AdminChainsListSchema } from '@/lib/schemas';
import { REGISTRY_KEYS, type RegistryKey } from '@/lib/operator/types';
import type { RegistryRow, RegistrySnapshot } from './RegistryPageClient';

export interface SnapshotDeps {
  readonly edgeBaseUrl: string;
  readonly fetchImpl?: typeof fetch;
  readonly cookieHeader?: string;
}

/**
 * Load a registry snapshot for SSR. NEVER throws — every failure mode is
 * encoded in the returned union so the Server Component can render 200.
 *
 *   - 401/403 → AUTH_REQUIRED (client renders sign-in prompt)
 *   - other non-OK → UNAVAILABLE { reason }
 *   - network error → UNAVAILABLE { reason: error message }
 *   - shape mismatch → UNAVAILABLE { reason: 'schema_mismatch' }
 *   - registry key without admin endpoint → UNAVAILABLE { reason: 'not_wired' }
 */
export async function loadRegistrySnapshot(
  key: RegistryKey,
  deps: SnapshotDeps,
): Promise<RegistrySnapshot> {
  if (!REGISTRY_KEYS.includes(key)) {
    return { status: 'UNAVAILABLE', reason: 'unknown_registry' };
  }
  // Only `chain` has a productive admin endpoint today. The other 11 are
  // fail-honest UNAVAILABLE until their backends land — never mocked.
  if (key !== 'chain') {
    return { status: 'UNAVAILABLE', reason: 'registry_not_wired' };
  }

  const fetchImpl = deps.fetchImpl ?? fetch;
  const url = `${deps.edgeBaseUrl}/api/admin/chains`;
  try {
    const headers: Record<string, string> = { accept: 'application/json' };
    if (deps.cookieHeader) headers['cookie'] = deps.cookieHeader;
    const res = await fetchImpl(url, { cache: 'no-store', headers });
    if (res.status === 401 || res.status === 403) {
      return { status: 'AUTH_REQUIRED' };
    }
    if (!res.ok) {
      return { status: 'UNAVAILABLE', reason: `upstream_http_${res.status}` };
    }
    const raw: unknown = await res.json().catch(() => null);
    const parsed = AdminChainsListSchema.safeParse(raw);
    if (!parsed.success) {
      return { status: 'UNAVAILABLE', reason: 'schema_mismatch' };
    }
    const data: readonly RegistryRow[] = parsed.data.items.map((c) => ({
      id: String(c.chain_id),
      enabled: c.enabled,
      config_hash: c.config_hash ?? 'pending',
    }));
    return { status: 'OK', data };
  } catch (e) {
    return { status: 'UNAVAILABLE', reason: (e as Error).message };
  }
}
