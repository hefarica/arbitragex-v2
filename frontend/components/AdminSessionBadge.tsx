/**
 * AdminSessionBadge — operator session indicator.
 *
 * Renders ANON or ADMIN+TTL depending on whether the V-AT-1 companion TTL
 * cookie (arbx_admin_session_ttl) is present and valid. Logout posts to
 * /admin/session/logout (which clears the httpOnly cookie server-side) AND
 * clears the local companion cookie via clearAdminToken().
 *
 * R1 (Mounted Snapshot): the first paint renders the ANON variant because
 * document.cookie is not readable on the server. After mount we read the
 * companion cookie and re-render. No hydration mismatch because both server
 * and pre-mount client render identically.
 *
 * Mirror Law Extendida: only composes Badge, Button (shadcn), Link (next).
 * Does NOT touch layout.tsx, globals.css, or ui/* primitives.
 */
'use client';

import React, { useCallback, useEffect, useState } from 'react';
import Link from 'next/link';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  adminTokenExpiresInMs,
  clearAdminToken,
  fmtRemaining,
  hasAdminSession,
} from '@/lib/admin-token';

const POLL_MS = 30_000;

export function AdminSessionBadge(): JSX.Element {
  const [mounted, setMounted] = useState(false);
  const [active, setActive] = useState(false);
  const [remainingMs, setRemainingMs] = useState(0);

  const refresh = useCallback(() => {
    const has = hasAdminSession();
    setActive(has);
    setRemainingMs(has ? adminTokenExpiresInMs() : 0);
  }, []);

  useEffect(() => {
    setMounted(true);
    refresh();
    const id = setInterval(refresh, POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  const onLogout = useCallback(async () => {
    await clearAdminToken();
    refresh();
    if (typeof window !== 'undefined') {
      window.location.reload();
    }
  }, [refresh]);

  // Pre-mount paint matches SSR (which has no document.cookie) — render ANON.
  if (!mounted || !active) {
    return (
      <div className="flex items-center gap-2" data-testid="admin-session-badge-anon">
        <Badge variant="secondary">ANON</Badge>
        <Link href="/admin/signin" className="text-xs underline text-muted-foreground">
          Sign in
        </Link>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2" data-testid="admin-session-badge-admin">
      <Badge
        variant="default"
        className="bg-green-600 hover:bg-green-700"
        data-testid="admin-session-badge-status"
      >
        ADMIN
      </Badge>
      <span className="text-xs text-muted-foreground" data-testid="admin-session-badge-ttl">
        expires in {fmtRemaining(remainingMs)}
      </span>
      <Button
        size="sm"
        variant="ghost"
        onClick={() => void onLogout()}
        data-testid="admin-session-badge-logout"
      >
        Logout
      </Button>
    </div>
  );
}
