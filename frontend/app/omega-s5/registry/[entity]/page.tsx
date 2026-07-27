/**
 * /omega-s5/registry/[entity] — Server Component shell (R8 fail-honest).
 *
 * Mounted Snapshot R1: this RSC fetches the registry list at SSR time and
 * passes a discriminated-union snapshot to the client component. The page
 * NEVER throws on 401/403 — those map to AUTH_REQUIRED so the operator
 * sees a sign-in prompt instead of HTTP 500.
 *
 * V-AT-1: server-side fetch lacks the operator's browser cookie, so a
 * snapshot of AUTH_REQUIRED is expected for unauthenticated visitors. The
 * client immediately retries with credentials:"include" so a logged-in
 * operator never has to refresh manually.
 */
import { notFound, redirect } from 'next/navigation';

import { REGISTRY_KEYS, type RegistryKey } from '@/lib/operator/types';
import { getApiBaseUrl } from '@/lib/api-client';

import { RegistryPageClient } from './RegistryPageClient';
import { loadRegistrySnapshot } from './snapshot';

export const dynamic = 'force-dynamic';
export const revalidate = 0;

interface PageProps {
  params: Promise<{ entity: string }>;
}

// H7 fix: docs historically referenced plural registry URLs (e.g.
// /omega-s5/registry/pools) but the canonical entities are singular
// (chain|rpc|dex|token|pool|wallet|strategy|contract). Redirect a trailing-s
// plural to its singular form instead of a bare 404.
function pluralToSingular(entity: string): string | null {
  if (!entity.endsWith('s')) return null;
  const candidate = entity.slice(0, -1);
  return REGISTRY_KEYS.includes(candidate as RegistryKey) ? candidate : null;
}

export default async function RegistryPage(props: PageProps): Promise<JSX.Element> {
  const { entity } = await props.params;
  if (!REGISTRY_KEYS.includes(entity as RegistryKey)) {
    const singular = pluralToSingular(entity);
    if (singular) redirect(`/omega-s5/registry/${singular}`);
    notFound();
  }
  const registryKey = entity as RegistryKey;

  const edgeBaseUrl = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  const initialSnapshot = await loadRegistrySnapshot(registryKey, { edgeBaseUrl });

  return (
    <RegistryPageClient
      registryKey={registryKey}
      initialSnapshot={initialSnapshot}
    />
  );
}
