/**
 * AdminSessionBadge — SSR-deterministic ANON rendering.
 *
 * Same toolkit-alignment constraint as the other component tests: no jsdom,
 * we render via react-dom/server to validate the SSR markup. Mounted-state
 * behaviour (ADMIN badge, TTL, logout) requires browser APIs and is
 * exercised by the higher-level admin-token tests + the page-level
 * integration tests.
 */
import React from 'react';
import { describe, it, expect } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';

import { AdminSessionBadge } from '../AdminSessionBadge';

describe('AdminSessionBadge — SSR ANON variant', () => {
  it('renders ANON badge and sign-in link before mount', () => {
    const html = renderToStaticMarkup(<AdminSessionBadge />);
    expect(html).toMatch(/data-testid="admin-session-badge-anon"/);
    expect(html).toMatch(/ANON/);
    expect(html).toMatch(/href="\/admin\/signin"/);
    expect(html).toMatch(/Sign in/);
    // Must NOT render the ADMIN variant before mount — would leak session
    // presence through SSR markup.
    expect(html).not.toMatch(/data-testid="admin-session-badge-admin"/);
  });
});
