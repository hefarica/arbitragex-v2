import { getApiBaseUrl } from "@/lib/api-client";
import { OnboardingPhaseStub } from "@/features/onboarding/OnboardingPhaseStub";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default function Phase2Page() {
  return <OnboardingPhaseStub n={2} edgeUrl={getApiBaseUrl()} />;
}
