/**
 * Phase 5 Server Component — R1 Mounted Snapshot Pattern.
 */
import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Phase5Client } from "@/features/onboarding/Phase5Client";
import { getOnboardingStatus } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function Phase5Page() {
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

  return <Phase5Client initialSnapshot={res.data} />;
}
