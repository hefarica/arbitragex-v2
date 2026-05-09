/**
 * Phase 2 Server Component — R1 Mounted Snapshot Pattern.
 *
 * Fetches OnboardingStatus at request time and passes it as initialSnapshot
 * to the Client Component. No non-deterministic values emitted from server.
 */
import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Phase2Client } from "@/features/onboarding/Phase2Client";
import { getOnboardingStatus } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function Phase2Page() {
  const res = await getOnboardingStatus();

  if (!res.ok) {
    return (
      <Alert variant="destructive">
        <AlertCircleIcon />
        <AlertTitle>Could not load onboarding status</AlertTitle>
        <AlertDescription>
          <code className="font-mono text-xs">{res.error}</code>
        </AlertDescription>
      </Alert>
    );
  }

  return <Phase2Client initialSnapshot={res.data} />;
}
