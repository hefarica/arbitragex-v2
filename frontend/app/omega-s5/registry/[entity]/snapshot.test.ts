/**
 * SSR snapshot loader — R8 fail-honest contract.
 *
 * The loader must NEVER throw and must encode every failure mode as an
 * explicit union variant so the Server Component renders HTTP 200.
 */
import { describe, expect, it, vi } from 'vitest';

import { loadRegistrySnapshot } from './snapshot';

function makeFetch(response: Partial<Response> | Error): typeof fetch {
  return vi.fn(async () => {
    if (response instanceof Error) throw response;
    return response as Response;
  }) as unknown as typeof fetch;
}

function jsonResponse(body: unknown, status: number): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

const SAMPLE_PAYLOAD = {
  count: 1,
  ts: '2026-05-15T00:00:00.000Z',
  items: [
    {
      id: 1,
      chain_id: 1,
      name: 'ethereum',
      rpc_http_url: 'https://eth.example/v2/k',
      rpc_ws_url: null,
      native_currency: 'ETH',
      block_time_ms: 12000,
      enabled: true,
      config_hash: 'abc',
      notes: null,
      created_at: '2026-05-13T12:00:00.000Z',
      updated_at: '2026-05-13T12:00:00.000Z',
      created_by: null,
      updated_by: null,
    },
  ],
};

describe('loadRegistrySnapshot — R8 fail-honest', () => {
  it('returns AUTH_REQUIRED on upstream 401 — never throws', async () => {
    const fetchImpl = makeFetch(jsonResponse({ error: 'unauthorized' }, 401));
    const snap = await loadRegistrySnapshot('chain', {
      edgeBaseUrl: 'http://edge:8787',
      fetchImpl,
    });
    expect(snap.status).toBe('AUTH_REQUIRED');
  });

  it('returns AUTH_REQUIRED on upstream 403', async () => {
    const fetchImpl = makeFetch(jsonResponse({ error: 'forbidden' }, 403));
    const snap = await loadRegistrySnapshot('chain', {
      edgeBaseUrl: 'http://edge:8787',
      fetchImpl,
    });
    expect(snap.status).toBe('AUTH_REQUIRED');
  });

  it('returns OK with parsed rows on 200', async () => {
    const fetchImpl = makeFetch(jsonResponse(SAMPLE_PAYLOAD, 200));
    const snap = await loadRegistrySnapshot('chain', {
      edgeBaseUrl: 'http://edge:8787',
      fetchImpl,
    });
    expect(snap.status).toBe('OK');
    if (snap.status === 'OK') {
      expect(snap.data).toHaveLength(1);
      expect(snap.data[0]?.id).toBe('1');
      expect(snap.data[0]?.enabled).toBe(true);
      expect(snap.data[0]?.config_hash).toBe('abc');
    }
  });

  it('returns UNAVAILABLE on schema mismatch — never throws', async () => {
    const fetchImpl = makeFetch(jsonResponse({ wrong: 'shape' }, 200));
    const snap = await loadRegistrySnapshot('chain', {
      edgeBaseUrl: 'http://edge:8787',
      fetchImpl,
    });
    expect(snap.status).toBe('UNAVAILABLE');
    if (snap.status === 'UNAVAILABLE') {
      expect(snap.reason).toBe('schema_mismatch');
    }
  });

  it('returns UNAVAILABLE on network error — never throws', async () => {
    const fetchImpl = makeFetch(new Error('econnrefused'));
    const snap = await loadRegistrySnapshot('chain', {
      edgeBaseUrl: 'http://edge:8787',
      fetchImpl,
    });
    expect(snap.status).toBe('UNAVAILABLE');
    if (snap.status === 'UNAVAILABLE') {
      expect(snap.reason).toBe('econnrefused');
    }
  });

  it('returns UNAVAILABLE for registries without admin endpoint (e.g. dex)', async () => {
    const fetchImpl = makeFetch(new Error('should not be called'));
    const snap = await loadRegistrySnapshot('dex', {
      edgeBaseUrl: 'http://edge:8787',
      fetchImpl,
    });
    expect(snap.status).toBe('UNAVAILABLE');
    if (snap.status === 'UNAVAILABLE') {
      expect(snap.reason).toBe('registry_not_wired');
    }
  });

  it('returns UNAVAILABLE on upstream 5xx with status reason', async () => {
    const fetchImpl = makeFetch(jsonResponse({ error: 'boom' }, 503));
    const snap = await loadRegistrySnapshot('chain', {
      edgeBaseUrl: 'http://edge:8787',
      fetchImpl,
    });
    expect(snap.status).toBe('UNAVAILABLE');
    if (snap.status === 'UNAVAILABLE') {
      expect(snap.reason).toBe('upstream_http_503');
    }
  });
});
