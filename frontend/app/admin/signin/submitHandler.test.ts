/**
 * /admin/signin submit handler — V-AT-1 invariants.
 *
 * The handler MUST clear the input value before returning (success OR
 * failure), call the injected setAdminToken once, and never persist the
 * token in module-level state.
 */
import { describe, expect, it, vi } from 'vitest';

import { submitAdminToken } from './submitHandler';

describe('submitAdminToken', () => {
  it('calls setAdminToken with the input value and returns ok on success', async () => {
    const input = { value: 'super-secret-token' };
    const setAdminToken = vi.fn().mockResolvedValue(true);
    const result = await submitAdminToken(input, { setAdminToken });
    expect(result.ok).toBe(true);
    expect(setAdminToken).toHaveBeenCalledTimes(1);
    expect(setAdminToken).toHaveBeenCalledWith('super-secret-token');
  });

  it('clears the input value AFTER reading it (success path)', async () => {
    const input = { value: 'super-secret-token' };
    const setAdminToken = vi.fn().mockResolvedValue(true);
    await submitAdminToken(input, { setAdminToken });
    expect(input.value).toBe('');
  });

  it('clears the input value AFTER reading it (failure path)', async () => {
    const input = { value: 'wrong-token' };
    const setAdminToken = vi.fn().mockResolvedValue(false);
    const result = await submitAdminToken(input, { setAdminToken });
    expect(result.ok).toBe(false);
    expect(input.value).toBe('');
  });

  it('clears the input value AFTER reading it (network error path)', async () => {
    const input = { value: 'any-token' };
    const setAdminToken = vi.fn().mockRejectedValue(new Error('boom'));
    const result = await submitAdminToken(input, { setAdminToken });
    expect(result.ok).toBe(false);
    expect(input.value).toBe('');
  });

  it('returns a GENERIC error message — never echoes the token', async () => {
    const input = { value: 'sensitive-secret-12345' };
    const setAdminToken = vi.fn().mockResolvedValue(false);
    const result = await submitAdminToken(input, { setAdminToken });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).not.toContain('sensitive-secret-12345');
      expect(result.error).toMatch(/Sign-in failed/);
    }
  });

  it('clears input even when setAdminToken throws synchronously', async () => {
    const input = { value: 'secret' };
    const setAdminToken = vi.fn(() => {
      throw new Error('sync boom');
    });
    const result = await submitAdminToken(input, { setAdminToken });
    expect(result.ok).toBe(false);
    expect(input.value).toBe('');
  });
});
