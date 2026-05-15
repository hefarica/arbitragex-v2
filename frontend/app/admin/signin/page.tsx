/**
 * /admin/signin — Sovereign sign-in page.
 *
 * V-AT-1 contract: the input value is sent to the edge via setAdminToken().
 * The edge validates it against upstream and sets the arbx_admin_session
 * httpOnly cookie that JS can never read. The input is cleared after every
 * submit attempt (success OR failure) so the secret leaves the JS heap as
 * fast as React's reconciler allows. We do NOT keep the value in any state
 * beyond the submit handler's local scope — no localStorage, no
 * sessionStorage, no module-level variable.
 *
 * Errors are intentionally generic — never echo the token, never reveal
 * whether the value was empty, malformed, or simply incorrect.
 */
'use client';

import React, { useRef, useState } from 'react';
import { useRouter } from 'next/navigation';

import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { setAdminToken } from '@/lib/admin-token';

import { submitAdminToken } from './submitHandler';

export default function AdminSigninPage(): JSX.Element {
  const router = useRouter();
  const inputRef = useRef<HTMLInputElement>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent<HTMLFormElement>): Promise<void> {
    e.preventDefault();
    if (submitting) return;
    const input = inputRef.current;
    if (!input) return;
    setSubmitting(true);
    setError(null);
    const result = await submitAdminToken(input, { setAdminToken });
    setSubmitting(false);
    if (result.ok) {
      router.push('/admin/chains');
      router.refresh();
      return;
    }
    setError(result.error);
  }

  return (
    <div className="mx-auto max-w-md py-10">
      <Card>
        <CardHeader>
          <CardTitle>Operator console — sign in</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-4" data-testid="admin-signin-form">
            <div className="space-y-2">
              <Label htmlFor="admin-token-input">Admin token</Label>
              <Input
                id="admin-token-input"
                ref={inputRef}
                type="password"
                autoComplete="off"
                spellCheck={false}
                required
                data-testid="admin-token-input"
                disabled={submitting}
              />
            </div>
            {error && (
              <p
                className="text-sm text-destructive"
                role="alert"
                data-testid="admin-signin-error"
              >
                {error}
              </p>
            )}
            <Button type="submit" disabled={submitting} data-testid="admin-signin-submit">
              {submitting ? 'Signing in…' : 'Sign in'}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
