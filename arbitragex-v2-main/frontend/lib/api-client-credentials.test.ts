/**
 * V-AT-1: every admin-routed fetch must carry credentials:"include" so the
 * httpOnly arbx_admin_session cookie crosses the SSH-tunnel port boundary.
 * This test pins the behaviour of the shared getValidated() helper.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { getAdminChains } from './api-client';

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

const EMPTY_LIST = { count: 0, items: [], ts: '2026-05-15T00:00:00.000Z' };

describe('api-client credentials wiring (V-AT-1)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('getValidated sends credentials:"include" via getAdminChains', async () => {
    const fetchSpy = vi
      .spyOn(global, 'fetch')
      .mockResolvedValue(jsonResponse(EMPTY_LIST));
    await getAdminChains();
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    const [, init] = fetchSpy.mock.calls[0]!;
    expect((init as RequestInit).credentials).toBe('include');
  });
});
