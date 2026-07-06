"use client";

/**
 * SiweAuthPanel — Sign-In With Ethereum (EIP-4361) IDENTITY-ONLY flow.
 *
 * Flow: GET nonce → build SIWE message (siwe lib) → wallet signs the message
 * (useSignMessage — a plain personal_sign, NOT a transaction, NOT blind) →
 * POST /api/auth/siwe/verify → server sets an httpOnly identity cookie. The
 * session proves address control and grants NOTHING capital-adjacent.
 *
 * SECURITY: signing here is a human-readable SIWE message (no blind signing).
 * No transaction is ever signed or sent. The session is identity-only.
 *
 * SSR-safe: gated behind a mounted flag; the disconnected shell is stable.
 */

import { useEffect, useState } from "react";
import { useAccount, useChainId, useSignMessage } from "wagmi";
import { SiweMessage } from "siwe";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { getApiBaseUrl } from "@/lib/api-client";

interface SessionState {
  authenticated: boolean;
  address?: string;
  chain_id?: number;
  reason?: string;
  session_mode?: string;
}

export function SiweAuthPanel() {
  const [mounted, setMounted] = useState(false);
  const [session, setSession] = useState<SessionState | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const { address, isConnected } = useAccount();
  const chainId = useChainId();
  const { signMessageAsync } = useSignMessage();

  async function refreshSession(): Promise<void> {
    try {
      const base = getApiBaseUrl();
      const r = await fetch(`${base}/api/auth/session`, { credentials: "include", headers: { accept: "application/json" } });
      const j = (await r.json()) as SessionState;
      setSession(j);
    } catch (e) {
      setStatus(`session read failed: ${(e as Error).message}`);
    }
  }

  useEffect(() => {
    setMounted(true);
    void refreshSession();
  }, []);

  async function signIn(): Promise<void> {
    if (!address) return;
    setBusy(true);
    setStatus(null);
    try {
      const base = getApiBaseUrl();
      // 1. one-time nonce from the server.
      const nonceRes = await fetch(`${base}/api/auth/siwe/nonce`, { credentials: "include", headers: { accept: "application/json" } });
      const nonceJson = (await nonceRes.json()) as { status: string; nonce?: string; reason?: string };
      if (nonceJson.status !== "ok" || !nonceJson.nonce) {
        setStatus(`nonce unavailable: ${nonceJson.reason ?? "unknown"}`);
        return;
      }
      // 2. build a human-readable SIWE message (no blind signing).
      const message = new SiweMessage({
        domain: window.location.host,
        address,
        statement: "Sign in to QuantumX (identity only — read-only, no capital, no broadcast).",
        uri: window.location.origin,
        version: "1",
        chainId: chainId,
        nonce: nonceJson.nonce,
      }).prepareMessage();
      // 3. wallet signs the message string (personal_sign).
      const signature = await signMessageAsync({ message });
      // 4. server verifies via the siwe library and sets the identity cookie.
      const verifyRes = await fetch(`${base}/api/auth/siwe/verify`, {
        method: "POST",
        credentials: "include",
        headers: { "content-type": "application/json", accept: "application/json" },
        body: JSON.stringify({ message, signature }),
      });
      const verifyJson = (await verifyRes.json()) as { status: string; reason?: string };
      if (verifyJson.status === "ok") {
        setStatus("Signed in (identity only).");
        await refreshSession();
      } else {
        setStatus(`sign-in failed: ${verifyJson.reason ?? "unknown"}`);
      }
    } catch (e) {
      setStatus(`sign-in error: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  }

  async function signOut(): Promise<void> {
    setBusy(true);
    try {
      const base = getApiBaseUrl();
      await fetch(`${base}/api/auth/logout`, { method: "POST", credentials: "include" });
      setStatus("Signed out.");
      await refreshSession();
    } catch (e) {
      setStatus(`sign-out error: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card data-testid="siwe-auth-panel">
      <CardHeader>
        <CardTitle className="flex items-center justify-between">
          <span>Sign-In With Ethereum (identity only)</span>
          <Badge variant="info">No capital · No broadcast</Badge>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <p className="text-sm text-muted-foreground">
          SIWE proves you control this address. It is a session for identity only —{" "}
          <code className="font-mono">session_mode: wallet_identity_only</code>. It signs a
          human-readable message (never a transaction, never blind), and grants no capital authority.
        </p>

        <div className="flex flex-wrap items-center gap-3" suppressHydrationWarning>
          {!mounted ? (
            <Badge variant="secondary">…</Badge>
          ) : session?.authenticated ? (
            <>
              <Badge variant="success" data-testid="siwe-authenticated">Authenticated (identity)</Badge>
              <span className="font-mono text-xs">{session.address}</span>
              <Button type="button" variant="outline" size="sm" disabled={busy} onClick={signOut} data-testid="siwe-signout">
                Sign out
              </Button>
            </>
          ) : (
            <>
              <Badge variant="secondary" data-testid="siwe-unauthenticated">Not authenticated</Badge>
              <Button
                type="button"
                variant="default"
                size="sm"
                disabled={busy || !mounted || !isConnected || !address}
                onClick={signIn}
                data-testid="siwe-signin"
              >
                {busy ? "Signing…" : "Sign in with Ethereum"}
              </Button>
              {mounted && !isConnected && (
                <span className="text-xs text-muted-foreground">Connect a wallet first.</span>
              )}
            </>
          )}
        </div>

        {session && !session.authenticated && session.reason && (
          <p className="text-xs text-muted-foreground/70">
            session unavailable: <code className="font-mono">{session.reason}</code>
          </p>
        )}
        {status && <p className="text-xs text-muted-foreground" data-testid="siwe-status">{status}</p>}
      </CardContent>
    </Card>
  );
}
